thread_local! {
    static SPAN_STACK: std::cell::UnsafeCell<Vec<&'static GuardInner>> = std::cell::UnsafeCell::new(Vec::with_capacity(1024));
}

#[inline]
pub fn new_span_root<T: Into<u32>>(tx: crate::CollectorTx, tag: T) -> SpanGuard {
    let root_time = crate::time::InstantMillis::now();
    let info = SpanInfo {
        id: crate::SpanID::new(),
        link: Link::Root(crate::time::real_time_ms()),
        tag: tag.into(),
    };
    SpanGuard(Some(GuardInner {
        root_time,
        elapsed_start: 0u32,
        tx: Some(tx),
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
        let tx = parent.tx.as_ref().unwrap().try_clone();

        if tx.is_err() {
            return SpanGuard(None);
        }

        let info = SpanInfo {
            id: crate::SpanID::new(),
            link: Link::Parent(parent.info.id),
            tag: tag.into(),
        };

        SpanGuard(Some(GuardInner {
            root_time,
            elapsed_start: root_time.elapsed(),
            tx: Some(tx.unwrap()),
            info,
        }))
    } else {
        SpanGuard(None)
    }
}

#[cfg(feature = "fine-async")]
#[inline]
pub(crate) fn new_span_continue(
    cont: Option<(
        u32,
        crate::SpanID,
        crate::time::InstantMillis,
        crate::CollectorTx,
    )>,
) -> SpanGuard {
    if let Some((tag, cont, root_time, tx)) = cont {
        let info = SpanInfo {
            id: crate::SpanID::new(),
            link: Link::Continue(cont),
            tag,
        };

        SpanGuard(Some(GuardInner {
            root_time,
            elapsed_start: root_time.elapsed(),
            tx: Some(tx),
            info,
        }))
    } else {
        SpanGuard(None)
    }
}

pub(crate) struct GuardInner {
    pub(crate) root_time: crate::time::InstantMillis,
    pub(crate) info: SpanInfo,
    elapsed_start: u32,
    pub(crate) tx: Option<crate::CollectorTx>,
}
impl !Sync for GuardInner {}

pub(crate) struct SpanInfo {
    pub(crate) id: crate::SpanID,
    link: Link,
    pub(crate) tag: u32,
}

enum Link {
    /// real time ms
    Root(u64),

    /// parent id
    Parent(crate::SpanID),

    #[cfg(feature = "fine-async")]
    Continue(crate::SpanID),
}

impl GuardInner {
    #[inline]
    pub fn enter(&self) -> Entered<'_> {
        Entered::new(self)
    }
}

impl Drop for GuardInner {
    fn drop(&mut self) {
        let span = crate::Span {
            id: self.info.id.into(),
            link: match self.info.link {
                Link::Root(ms) => crate::Link::Root { start_time_ms: ms },
                Link::Parent(id) => crate::Link::Parent { id: id.into() },
                #[cfg(feature = "fine-async")]
                Link::Continue(id) => crate::Link::Continue { id: id.into() },
            },
            elapsed_start: self.elapsed_start,
            elapsed_end: self.root_time.elapsed(),
            tag: self.info.tag,
        };
        self.tx.take().unwrap().put(span);
    }
}

pub struct SpanGuard(pub(crate) Option<GuardInner>);

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
    #[inline]
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

    #[derive(Copy, Clone)]
    enum CollectorType {
        Bounded,
        Unbounded,
    }

    // An auxiliary function for checking relations of spans.
    // Note that the tags of each spans cannot be the same.
    //
    // Return: [(tag, Option<parent_tag>)], sorted by tag
    fn rebuild_relation_by_tag(spans: Vec<crate::Span>) -> Vec<(u32, Option<u32>)> {
        let infos: std::collections::HashMap<u32, (Option<u32>, u32)> = spans
            .into_iter()
            .map(|s| {
                (
                    s.id,
                    (
                        match s.link {
                            crate::Link::Root { .. } => None,
                            crate::Link::Parent { id } => Some(id),
                            #[cfg(feature = "fine-async")]
                            crate::Link::Continue { .. } => unreachable!(),
                        },
                        s.tag,
                    ),
                )
            })
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

    fn root(tag: u32, collector_type: CollectorType) -> (SpanGuard, crate::CollectorRx) {
        let (tx, rx) = match collector_type {
            CollectorType::Bounded => crate::Collector::bounded(1024),
            CollectorType::Unbounded => crate::Collector::unbounded(),
        };
        let root = new_span_root(tx, tag);
        (root, rx)
    }

    fn sync_spanned(tag: u32) {
        let span = new_span(tag);
        let _g = span.enter();
    }

    #[test]
    fn span_basic() {
        for clt_type in &[CollectorType::Bounded, CollectorType::Unbounded] {
            let (root, mut rx) = root(0, *clt_type);
            {
                let root = root;
                let _g = root.enter();

                sync_spanned(1);
            }

            let spans = rx.collect().unwrap();
            let spans = rebuild_relation_by_tag(spans);

            assert_eq!(spans.len(), 2);
            assert_eq!(&spans, &[(0, None), (1, Some(0))]);
            assert_eq!(SPAN_STACK.with(|stack| unsafe { (&*stack.get()).len() }), 0);
        }
    }

    #[test]
    fn span_wide_function() {
        for clt_type in &[CollectorType::Bounded, CollectorType::Unbounded] {
            let (root, mut rx) = root(0, *clt_type);
            {
                let root = root;
                let _g = root.enter();

                for i in 1..=10 {
                    sync_spanned(i);
                }
            }

            let spans = rx.collect().unwrap();
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
    }

    #[test]
    fn span_deep_function() {
        fn sync_spanned_rec_tag_step_to_1(step: u32) {
            let span = new_span(step);
            let _g = span.enter();

            if step > 1 {
                sync_spanned_rec_tag_step_to_1(step - 1);
            }
        }

        for clt_type in &[CollectorType::Bounded, CollectorType::Unbounded] {
            let (root, mut rx) = root(0, *clt_type);
            {
                let root = root;
                let _g = root.enter();

                sync_spanned_rec_tag_step_to_1(10);
            }

            let spans = rx.collect().unwrap();
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

    #[test]
    fn test_bounded() {
        let (tx, mut rx) = crate::Collector::bounded(2);

        {
            let s = new_span_root(tx, 0u32);
            let _g = s.enter();

            {
                // new span successfully
                let s = new_span(1u32);
                let _g = s.enter();
            }

            {
                // collector is full, failed
                let s = new_span(2u32);
                let _g = s.enter();
            }

            {
                // collector is full, failed
                let s = new_span(3u32);
                let _g = s.enter();
            }

            {
                // collector is full, failed
                let s = new_span(4u32);
                let _g = s.enter();
            }
        }

        let spans = rx.collect().unwrap();
        let spans = rebuild_relation_by_tag(spans);

        assert_eq!(spans.len(), 2);
        assert_eq!(&spans, &[(0, None), (1, Some(0))]);
        assert_eq!(SPAN_STACK.with(|stack| unsafe { (&*stack.get()).len() }), 0);
    }
}
