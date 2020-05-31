type SpanId = u64;

thread_local! {
    pub(crate) static TRACE_LOCAL: std::cell::UnsafeCell<TraceLocal> = std::cell::UnsafeCell::new(TraceLocal {
        span_stack: Vec::with_capacity(1024),
        enter_stack: Vec::with_capacity(1024),
        id_prefix: next_global_id_prefix(),
        id_suffix: 0,
        cur_collector: None,
    });
}

pub(crate) struct TraceLocal {
    pub(crate) span_stack: Vec<crate::Span>,
    pub(crate) enter_stack: Vec<SpanId>,
    pub(crate) id_prefix: u32,
    pub(crate) id_suffix: u32,
    pub(crate) cur_collector: Option<std::sync::Arc<crate::collector::CollectorInner>>,
}

static GLOBAL_ID_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[inline]
fn next_global_id_prefix() -> u32 {
    GLOBAL_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

pub struct LocalTraceGuard {
    collector: std::sync::Arc<crate::collector::CollectorInner>,
    trace_local: *mut TraceLocal,
    start_index: usize,
    start_time_ns: u64,
}

impl !Sync for LocalTraceGuard {}
impl !Send for LocalTraceGuard {}

impl LocalTraceGuard {
    pub(crate) fn new<T: Into<u32>>(
        collector: std::sync::Arc<crate::collector::CollectorInner>,
        event: T,
        link: crate::Link,
        start_time_ns: u64,
    ) -> Option<(Self, SpanId)> {
        if collector.closed.load(std::sync::atomic::Ordering::SeqCst) {
            return None;
        }

        let trace_local = TRACE_LOCAL.with(|trace_local| trace_local.get());
        let tl = unsafe { &mut *trace_local };

        tl.cur_collector = Some(collector.clone());

        let id = {
            if tl.id_suffix == std::u32::MAX {
                tl.id_suffix = 0;
                tl.id_prefix = next_global_id_prefix();
            } else {
                tl.id_suffix += 1;
            }
            ((tl.id_prefix as u64) << 32) | tl.id_suffix as u64
        };

        tl.enter_stack.push(id);
        let start_index = tl.span_stack.len();

        tl.span_stack.push(crate::Span {
            id,
            link,
            begin_cycles: crate::time::monotonic_cycles(),
            end_cycles: 0,
            event: event.into(),
        });

        Some((
            Self {
                collector,
                trace_local,
                start_index,
                start_time_ns,
            },
            id,
        ))
    }
}

impl Drop for LocalTraceGuard {
    fn drop(&mut self) {
        let tl = unsafe { &mut *self.trace_local };

        tl.span_stack[self.start_index].end_cycles = crate::time::monotonic_cycles();
        let id = tl.span_stack[self.start_index].id;

        assert_eq!(tl.enter_stack.pop().unwrap(), id, "corrupted stack");

        if !self
            .collector
            .closed
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            self.collector.queue.push(crate::SpanSet {
                start_time_ns: self.start_time_ns,
                cycles_per_sec: crate::time::cycles_per_sec(),
                spans: tl.span_stack[self.start_index..].to_vec(),
            });
        }

        tl.span_stack.truncate(self.start_index);
        if tl.span_stack.capacity() > 1024 && tl.span_stack.len() < 512 {
            tl.span_stack.shrink_to(1024);
        }
        if tl.enter_stack.capacity() > 1024 && tl.enter_stack.len() < 512 {
            tl.enter_stack.shrink_to(1024);
        }

        tl.cur_collector = None;
    }
}

pub struct SpanGuard {
    trace_local: *mut TraceLocal,
    index: usize,
}

impl !Sync for SpanGuard {}
impl !Send for SpanGuard {}

impl SpanGuard {
    pub(crate) fn new(event: u32) -> Option<Self> {
        let trace_local = TRACE_LOCAL.with(|trace_local| trace_local.get());
        let tl = unsafe { &mut *trace_local };

        if tl.cur_collector.is_none() || tl.enter_stack.is_empty() {
            return None;
        }

        let index = tl.span_stack.len();
        let parent = *tl.enter_stack.last().unwrap();

        let id = {
            if tl.id_suffix == std::u32::MAX {
                tl.id_suffix = 0;
                tl.id_prefix = next_global_id_prefix();
            } else {
                tl.id_suffix += 1;
            }
            ((tl.id_prefix as u64) << 32) | tl.id_suffix as u64
        };

        tl.enter_stack.push(id);

        tl.span_stack.push(crate::Span {
            id,
            link: crate::Link::Parent { id: parent },
            begin_cycles: crate::time::monotonic_cycles(),
            end_cycles: 0,
            event,
        });

        Some(Self { trace_local, index })
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        let tl = unsafe { &mut *self.trace_local };
        tl.span_stack[self.index].end_cycles = crate::time::monotonic_cycles();
        tl.enter_stack.pop();
    }
}
