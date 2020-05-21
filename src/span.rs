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

#[inline]
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
            elapsed_start: root_time.elapsed_ms(),
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
            elapsed_start: root_time.elapsed_ms(),
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
            elapsed_end: self.root_time.elapsed_ms(),
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
        let id = self.guard.info.id;
        SPAN_STACK.with(|spans| unsafe {
            let stack = &mut *spans.get();
            let (idx, _) = stack
                .iter()
                .enumerate()
                .rev()
                .find(|(_, span)| span.info.id == id)
                .expect("corrupted stack");

            stack.remove(idx);
        })
    }
}
