mod collector;
pub mod future;
mod span_id;
pub mod util;
pub use tracer_attribute::trace;

pub use collector::*;
pub use span_id::SpanID;

pub const DEFAULT_COLLECTOR: CollectorType = CollectorType::Channel;

pub struct Span {
    pub id: SpanID,
    pub parent: Option<SpanID>,
    pub elapsed_start: u32,
    pub elapsed_end: u32,
    pub tag: u32,
}

thread_local! {
    static SPAN_STACK: std::cell::UnsafeCell<Vec<&'static SpanGuard>> = std::cell::UnsafeCell::new(Vec::with_capacity(1024));
}

#[inline]
pub fn new_span_root<T: Into<u32>>(tx: CollectorTx, tag: T) -> SpanGuard {
    let root_time = std::time::Instant::now();
    let info = SpanInfo {
        id: SpanID::new(),
        parent: None,
        tag: tag.into(),
    };

    SpanGuard {
        root_time,
        elapsed_start: 0u32,
        tx,
        info,
    }
}

#[inline]
pub fn new_span<T: Into<u32>>(tag: T) -> OSpanGuard {
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

        OSpanGuard(Some(SpanGuard {
            root_time,
            elapsed_start: root_time.elapsed().as_millis() as u32,
            tx,
            info,
        }))
    } else {
        OSpanGuard(None)
    }
}

pub struct SpanGuard {
    root_time: std::time::Instant,
    info: SpanInfo,
    elapsed_start: u32,
    tx: CollectorTx,
}

struct SpanInfo {
    id: SpanID,
    parent: Option<SpanID>,
    tag: u32,
}

impl SpanGuard {
    #[inline]
    pub fn enter(&self) -> Entered<'_> {
        Entered::new(self)
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        self.tx.push(Span {
            id: self.info.id,
            parent: self.info.parent,
            elapsed_start: self.elapsed_start,
            elapsed_end: self.root_time.elapsed().as_millis() as u32,
            tag: self.info.tag,
        });
    }
}

pub struct OSpanGuard(Option<SpanGuard>);

impl OSpanGuard {
    #[inline]
    pub fn enter(&self) -> Option<Entered<'_>> {
        self.0.as_ref().map(|s| s.enter())
    }
}

pub struct Entered<'a> {
    guard: &'a SpanGuard,
}

impl<'a> Entered<'a> {
    fn new(span_guard: &'a SpanGuard) -> Self {
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
