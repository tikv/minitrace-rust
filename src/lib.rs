mod collector;
pub mod future;
mod span_id;
pub mod time;
pub mod util;

pub use minitrace_attribute::trace;

pub use collector::*;
pub use span_id::SpanID;

pub const DEFAULT_COLLECTOR: CollectorType = CollectorType::Channel;

pub struct Span {
    pub id: std::num::NonZeroU32,
    pub parent_id: Option<std::num::NonZeroU32>,
    pub elapsed_start: u32,
    pub elapsed_end: u32,
    pub tag: u32,
}

thread_local! {
    static SPAN_STACK: std::cell::UnsafeCell<Vec<&'static GuardInner>> = std::cell::UnsafeCell::new(Vec::with_capacity(1024));
}

#[inline]
pub fn new_span_root<T: Into<u32>>(tx: CollectorTx, tag: T) -> SpanGuard {
    let root_time = time::Instant::now_coarse();
    let info = SpanInfo {
        id: SpanID::new(),
        parent: None,
        tag: tag.into(),
    };
    SpanGuard(Some(GuardInner {
        root_time,
        elapsed_start: 0u32,
        tx,
        info,
    }))
}

pub fn none() -> SpanGuard {
    SpanGuard(None)
}

#[inline]
pub fn new_span<T: Into<u32>>(tag: T) -> SpanGuard {
    if let Some(parent) = SPAN_STACK.with(|spans| unsafe {
        let spans = &*spans.get();
        spans.last()
    }) {
        let root_time = parent.root_time;
        let tx = parent.tx.clone();

        let info = SpanInfo {
            id: SpanID::new(),
            parent: Some(parent.info.id),
            tag: tag.into(),
        };

        SpanGuard(Some(GuardInner {
            root_time,
            elapsed_start: time::duration_to_ms(root_time.elapsed()),
            tx,
            info,
        }))
    } else {
        SpanGuard(None)
    }
}

pub struct GuardInner {
    root_time: time::Instant,
    info: SpanInfo,
    elapsed_start: u32,
    tx: CollectorTx,
}

struct SpanInfo {
    id: SpanID,
    parent: Option<SpanID>,
    tag: u32,
}

impl GuardInner {
    #[inline]
    pub fn enter(&self) -> Entered<'_> {
        Entered::new(self)
    }
}

impl Drop for GuardInner {
    fn drop(&mut self) {
        self.tx.push(Span {
            id: self.info.id.into(),
            parent_id: self.info.parent.map(Into::into),
            elapsed_start: self.elapsed_start,
            elapsed_end: time::duration_to_ms(self.root_time.elapsed()),
            tag: self.info.tag,
        });
    }
}

pub struct SpanGuard(Option<GuardInner>);

impl SpanGuard {
    #[inline]
    pub fn enter(&self) -> Option<Entered<'_>> {
        self.0.as_ref().map(|s| s.enter())
    }
}

pub struct Entered<'a> {
    guard: &'a GuardInner,
}

impl<'a> Entered<'a> {
    fn new(span_guard: &'a GuardInner) -> Self {
        SPAN_STACK
            .with(|spans| unsafe { (&mut *spans.get()).push(std::mem::transmute(span_guard)) });
        Entered { guard: span_guard }
    }
}

impl Drop for Entered<'_> {
    fn drop(&mut self) {
        let guard = SPAN_STACK
            .with(|spans| unsafe { (&mut *spans.get()).pop() })
            .expect("corrupted stack");

        assert_eq!(guard.info.id, self.guard.info.id, "corrupted stack");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // An auxiliary function for checking relations of spans.
    // Note that the tags of each spans cannot be the same.
    //
    // Return: [(tag, Option<parent_tag>)], sorted by tag
    fn rebuild_relation_by_tag(spans: Vec<Span>) -> Vec<(u32, Option<u32>)> {
        let infos: std::collections::HashMap<u32, (Option<u32>, u32)> = spans
            .into_iter()
            .map(|s| (s.id.into(), (s.parent_id.map(Into::into), s.tag)))
            .collect();

        let mut res = Vec::with_capacity(infos.len());

        for (_id, (parent_id, tag)) in infos.iter() {
            if let Some(p) = parent_id {
                res.push((*tag, Some(infos[&p].1)));
            } else {
                res.push((*tag, None));
            }
        }

        res.sort();
        res
    }

    fn root(tag: u32) -> (SpanGuard, CollectorRx) {
        let (tx, rx) = Collector::new_default();
        let root = new_span_root(tx, tag);
        (root, rx)
    }

    fn sync_spanned(tag: u32) {
        let span = new_span(tag);
        let _g = span.enter();
    }

    #[test]
    fn span_basic() {
        let (root, rx) = root(0);
        {
            let root = root;
            let _g = root.enter();

            sync_spanned(1);
        }

        let spans = rx.collect();
        let spans = rebuild_relation_by_tag(spans);

        assert_eq!(spans.len(), 2);
        assert_eq!(&spans, &[(0, None), (1, Some(0))]);
        assert_eq!(SPAN_STACK.with(|stack| unsafe { (&*stack.get()).len() }), 0);
    }

    #[test]
    fn span_wide_function() {
        let (root, rx) = root(0);
        {
            let root = root;
            let _g = root.enter();

            for i in 1..11 {
                sync_spanned(i);
            }
        }

        let spans = rx.collect();
        let spans = rebuild_relation_by_tag(spans);

        assert_eq!(spans.len(), 11);
        assert_eq!(
            &spans,
            &[
                (0, None),
                (1, Some(0)),
                (2, Some(0)),
                (3, Some(0)),
                (4, Some(0)),
                (5, Some(0)),
                (6, Some(0)),
                (7, Some(0)),
                (8, Some(0)),
                (9, Some(0)),
                (10, Some(0))
            ]
        );
        assert_eq!(SPAN_STACK.with(|stack| unsafe { (&*stack.get()).len() }), 0);
    }

    #[test]
    fn span_deep_function() {
        fn sync_spanned_rec_tag_step_to_1(step: u32) {
            if step == 0 {
                return;
            } else {
                let span = new_span(step);
                let _g = span.enter();
                sync_spanned_rec_tag_step_to_1(step - 1);
            }
        }

        let (root, rx) = root(0);
        {
            let root = root;
            let _g = root.enter();

            sync_spanned_rec_tag_step_to_1(10);
        }

        let spans = rx.collect();
        let spans = rebuild_relation_by_tag(spans);

        assert_eq!(spans.len(), 11);
        assert_eq!(
            &spans,
            &[
                (0, None),
                (1, Some(2)),
                (2, Some(3)),
                (3, Some(4)),
                (4, Some(5)),
                (5, Some(6)),
                (6, Some(7)),
                (7, Some(8)),
                (8, Some(9)),
                (9, Some(10)),
                (10, Some(0))
            ]
        );
        assert_eq!(SPAN_STACK.with(|stack| unsafe { (&*stack.get()).len() }), 0);
    }
}
