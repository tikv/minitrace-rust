#[macro_use]
extern crate lazy_static;

pub mod future;
pub mod util;

pub use tracer_attribute;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct SpanID {
    slab_index: usize,
    elapsed_start: std::time::Duration,
}

#[derive(Debug)]
pub struct Span {
    pub tag: &'static str,
    pub id: SpanID,
    pub parent: Option<SpanID>,
    pub elapsed_start: std::time::Duration,
    pub elapsed_end: std::time::Duration,
}

pub struct SpanInner {
    tag: &'static str,
    parent: Option<SpanID>,
    root_time: std::time::Instant,
    elapsed_start: std::time::Duration,
}

thread_local! {
    static SPAN_STACK: std::cell::RefCell<Vec<&'static SpanGuard>> = std::cell::RefCell::new(Vec::with_capacity(1024));
}

lazy_static! {
    static ref REGISTRY: sharded_slab::Slab<SpanInner> = sharded_slab::Slab::new();
}

pub fn new_span_root(tag: &'static str, sender: crossbeam::channel::Sender<Span>) -> SpanGuard {
        let root_time = std::time::Instant::now();
        let elapsed_start = std::time::Duration::new(0, 0);
        let span = SpanInner {
            tag,
            parent: None,
            root_time,
            elapsed_start,
        };
        let slab_index = REGISTRY.insert(span).expect("full");

        let id = SpanID {
            slab_index,
            elapsed_start,
        };

        SpanGuard {
            id,
            root_time,
            sender,
        }
}

pub fn new_span(tag: &'static str) -> OSpanGuard {
    if let Some(parent) = SPAN_STACK.with(|span_idx| {
        let span_idx = span_idx.borrow();
        span_idx.last().cloned()
    }) {
        let root_time = parent.root_time;
        let elapsed_start = root_time.elapsed();
        let sender = parent.sender.clone();

        let slab_index = REGISTRY.insert(SpanInner {
            tag,
            parent: Some(parent.id),
            root_time,
            elapsed_start,
        }).expect("full");

        let id = SpanID {
            slab_index,
            elapsed_start,
        };

        OSpanGuard(Some(SpanGuard {
            id,
            root_time,
            sender,
        }))
    } else {
        OSpanGuard(None)
    }
}

pub struct SpanGuard {
    id: SpanID,
    root_time: std::time::Instant,
    sender: crossbeam::channel::Sender<Span>,
}

impl SpanGuard {
    pub fn enter(&self) -> Entered<'_> {
        SPAN_STACK.with(|spans| {
            spans
                .borrow_mut()
                .push(unsafe { std::mem::transmute(self) });
        });

        Entered { guard: self }
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        let span = REGISTRY.take(self.id.slab_index).expect("can not get span");

        let _ = self.sender.try_send(Span {
            tag: span.tag,
            id: self.id,
            parent: span.parent,
            elapsed_start: span.elapsed_start,
            elapsed_end: span.root_time.elapsed(),
        });
    }
}

pub struct OSpanGuard(Option<SpanGuard>);

impl OSpanGuard {
    pub fn enter(&self) -> Option<Entered<'_>> {
        self.0.as_ref().map(|s| s.enter())
    }
}

pub struct Entered<'a> {
    guard: &'a SpanGuard,
}

impl Drop for Entered<'_> {
    fn drop(&mut self) {
        let guard = SPAN_STACK
            .with(|spans| spans.borrow_mut().pop())
            .expect("corrupted stack");

        assert_eq!(guard.id, self.guard.id, "corrupted stack");
    }
}
