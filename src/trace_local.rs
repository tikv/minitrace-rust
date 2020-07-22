// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

type SpanId = u64;

const INIT_NORMAL_LEN: usize = 1024;
const INIT_BYTES_LEN: usize = 16384;

thread_local! {
    pub(crate) static TRACE_LOCAL: std::cell::UnsafeCell<TraceLocal> = std::cell::UnsafeCell::new(TraceLocal {
        spans: Vec::with_capacity(INIT_NORMAL_LEN),
        enter_stack: Vec::with_capacity(INIT_NORMAL_LEN),
        property_id_to_len: Vec::with_capacity(INIT_NORMAL_LEN),
        property_payload: Vec::with_capacity(INIT_BYTES_LEN),
        id_prefix: next_global_id_prefix(),
        id_suffix: 0,
        cur_collector: None,
    });
}

pub(crate) struct TraceLocal {
    pub(crate) spans: Vec<crate::Span>,
    pub(crate) enter_stack: Vec<SpanId>,
    pub(crate) property_id_to_len: Vec<(SpanId, u64)>,
    pub(crate) property_payload: Vec<u8>,
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
    create_time_ns: u64,
    start_time_ns: u64,

    span_start_index: usize,
    property_start_index: usize,
    property_payload_start_index: usize,
}

impl !Sync for LocalTraceGuard {}
impl !Send for LocalTraceGuard {}

impl LocalTraceGuard {
    pub(crate) fn new<T: Into<u32>>(
        collector: std::sync::Arc<crate::collector::CollectorInner>,
        event: T,
        link: crate::Link,
        create_time_ns: u64,
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
        let span_start_index = tl.spans.len();
        let property_start_index = tl.property_id_to_len.len();
        let property_payload_start_index = tl.property_payload.len();

        tl.spans.push(crate::Span {
            id,
            link,
            begin_cycles: minstant::now(),
            elapsed_cycles: 0,
            event: event.into(),
        });

        Some((
            Self {
                collector,
                trace_local,
                span_start_index,
                property_start_index,
                property_payload_start_index,
                create_time_ns,
                start_time_ns,
            },
            id,
        ))
    }
}

impl Drop for LocalTraceGuard {
    fn drop(&mut self) {
        let tl = unsafe { &mut *self.trace_local };

        // fill the elapsed cycles of the first span
        tl.spans[self.span_start_index].elapsed_cycles =
            minstant::now().saturating_sub(tl.spans[self.span_start_index].begin_cycles);

        // check if enter stack is corrupted
        let id = tl.spans[self.span_start_index].id;
        assert_eq!(tl.enter_stack.pop().unwrap(), id, "corrupted stack");

        if !self
            .collector
            .closed
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            let spans = tl.spans.split_off(self.span_start_index);
            let property_id_to_len = tl.property_id_to_len.split_off(self.property_start_index);
            let property_payload = tl
                .property_payload
                .split_off(self.property_payload_start_index);

            self.collector.queue.push(crate::SpanSet {
                create_time_ns: self.create_time_ns,
                start_time_ns: self.start_time_ns,
                spans,
                properties: crate::Properties {
                    span_id_to_len: property_id_to_len,
                    payload: property_payload,
                },
            });
        }

        // shrink all vectors in case they take up too much memory
        if tl.spans.capacity() > INIT_NORMAL_LEN && tl.spans.len() < INIT_NORMAL_LEN / 2 {
            tl.spans.shrink_to(INIT_NORMAL_LEN);
        }
        if tl.enter_stack.capacity() > INIT_NORMAL_LEN && tl.enter_stack.len() < INIT_NORMAL_LEN / 2
        {
            tl.enter_stack.shrink_to(INIT_NORMAL_LEN);
        }
        if tl.property_id_to_len.capacity() > INIT_NORMAL_LEN
            && tl.property_id_to_len.len() < INIT_NORMAL_LEN / 2
        {
            tl.property_id_to_len.shrink_to(INIT_NORMAL_LEN);
        }
        if tl.property_payload.capacity() > INIT_BYTES_LEN
            && tl.property_payload.len() < INIT_BYTES_LEN / 2
        {
            tl.property_payload.shrink_to(INIT_BYTES_LEN);
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

        let index = tl.spans.len();
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

        tl.spans.push(crate::Span {
            id,
            link: crate::Link::Parent { id: parent },
            begin_cycles: minstant::now(),
            elapsed_cycles: 0,
            event,
        });

        Some(Self { trace_local, index })
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        let tl = unsafe { &mut *self.trace_local };
        tl.spans[self.index].elapsed_cycles =
            minstant::now().saturating_sub(tl.spans[self.index].begin_cycles);
        tl.enter_stack.pop();
    }
}
