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
    pub id: u32,
    pub parent_id: Option<u32>,
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
