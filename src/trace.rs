// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;

use crate::{Span, State};

type SpanId = u64;

const INIT_NORMAL_LEN: usize = 1024;
const INIT_BYTES_LEN: usize = 16384;

thread_local! {
    pub static TRACE_LOCAL: std::cell::UnsafeCell<TraceLocal> = std::cell::UnsafeCell::new(TraceLocal {
        spans: Vec::with_capacity(INIT_NORMAL_LEN),
        enter_stack: Vec::with_capacity(INIT_NORMAL_LEN),
        property_ids: Vec::with_capacity(INIT_NORMAL_LEN),
        property_lens: Vec::with_capacity(INIT_NORMAL_LEN),
        property_payload: Vec::with_capacity(INIT_BYTES_LEN),
        id_prefix: next_global_id_prefix(),
        id_suffix: 0,
        cur_collector: None,
    });
}

pub struct TraceLocal {
    /// local span collector
    pub spans: Vec<crate::Span>,

    /// for parent-child relation construction
    pub enter_stack: Vec<SpanId>,

    /// local property collector
    pub property_ids: Vec<SpanId>,
    pub property_lens: Vec<u64>,
    pub property_payload: Vec<u8>,

    /// for id construction
    pub id_prefix: u32,
    pub id_suffix: u32,

    /// shared tracing collector
    pub cur_collector: Option<std::sync::Arc<crate::collector::CollectorInner>>,
}

static GLOBAL_ID_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[inline]
fn next_global_id_prefix() -> u32 {
    GLOBAL_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[must_use]
pub struct ScopeGuard {
    collector: std::sync::Arc<crate::collector::CollectorInner>,

    span_start_index: usize,
    property_start_index: usize,
    property_payload_start_index: usize,

    _marker: PhantomData<*const ()>,
}

pub struct LeadingSpan {
    pub state: State,
    pub related_id: SpanId,
    pub begin_cycles: u64,
    pub elapsed_cycles: u64,
    pub event: u32,
}

impl ScopeGuard {
    /// The `state` of a leading span is `Root` or `Spawning` or `Scheduling` which indicates
    /// the origin of the tracing context.
    /// The `elapsed_cycles` of a leading span is sorts of waiting time not executing time.
    /// Following a leading span, it's a span of `Settle` state, meaning traced execution is started.
    pub(crate) fn new(
        collector: std::sync::Arc<crate::collector::CollectorInner>,
        now_cycles: u64,
        LeadingSpan {
            state,
            related_id,
            begin_cycles,
            elapsed_cycles,
            event: pending_event,
        }: LeadingSpan,
        settle_event: u32,
    ) -> Option<(Self, SpanId)> {
        if collector.closed.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }

        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        if tl.cur_collector.is_some() {
            return None;
        }
        tl.cur_collector = Some(collector.clone());

        // fetch two ids, one for leading span, one for new span
        let (id0, id1) = {
            if tl.id_suffix >= std::u32::MAX - 1 {
                tl.id_suffix = 0;
                tl.id_prefix = next_global_id_prefix();
            } else {
                tl.id_suffix += 2;
            }
            let id = ((tl.id_prefix as u64) << 32) | tl.id_suffix as u64;
            (id - 1, id)
        };

        tl.spans.push(crate::Span {
            id: id0,
            state,
            related_id,
            begin_cycles,
            elapsed_cycles,
            event: pending_event,
        });
        tl.spans.push(crate::Span {
            id: id1,
            state: State::Settle,
            related_id: id0,
            begin_cycles: now_cycles,
            elapsed_cycles: 0,
            event: settle_event,
        });
        tl.enter_stack.push(id1);

        let span_start_index = tl.spans.len() - 2;
        let property_start_index = tl.property_ids.len();
        let property_payload_start_index = tl.property_payload.len();

        Some((
            Self {
                collector,
                span_start_index,
                property_start_index,
                property_payload_start_index,
                _marker: std::marker::PhantomData,
            },
            id0,
        ))
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        // fill the elapsed cycles of the first span (except the leading span)
        tl.spans[self.span_start_index + 1].elapsed_cycles =
            minstant::now().saturating_sub(tl.spans[self.span_start_index + 1].begin_cycles);

        // check if the enter stack is corrupted
        let id = tl.spans[self.span_start_index + 1].id;
        assert_eq!(tl.enter_stack.pop().unwrap(), id, "corrupted stack");

        if !self
            .collector
            .closed
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let spans = tl.spans.split_off(self.span_start_index);
            let property_ids = tl.property_ids.split_off(self.property_start_index);
            let property_lens = tl.property_lens.split_off(self.property_start_index);
            let property_payload = tl
                .property_payload
                .split_off(self.property_payload_start_index);

            self.collector.queue.push(crate::collector::SpanSet {
                spans,
                properties: crate::Properties {
                    span_ids: property_ids,
                    property_lens,
                    payload: property_payload,
                },
            });
        } else {
            tl.spans.truncate(self.span_start_index);
            tl.property_ids.truncate(self.property_start_index);
            tl.property_lens.truncate(self.property_start_index);
            tl.property_payload
                .truncate(self.property_payload_start_index);
        }

        // try to shrink all vectors in case they take up too much memory
        if tl.spans.capacity() > INIT_NORMAL_LEN && tl.spans.len() < INIT_NORMAL_LEN / 2 {
            tl.spans.shrink_to(INIT_NORMAL_LEN);
        }
        if tl.enter_stack.capacity() > INIT_NORMAL_LEN && tl.enter_stack.len() < INIT_NORMAL_LEN / 2
        {
            tl.enter_stack.shrink_to(INIT_NORMAL_LEN);
        }
        if tl.property_ids.capacity() > INIT_NORMAL_LEN
            && tl.property_ids.len() < INIT_NORMAL_LEN / 2
        {
            tl.property_ids.shrink_to(INIT_NORMAL_LEN);
            tl.property_lens.shrink_to(INIT_NORMAL_LEN);
        }
        if tl.property_payload.capacity() > INIT_BYTES_LEN
            && tl.property_payload.len() < INIT_BYTES_LEN / 2
        {
            tl.property_payload.shrink_to(INIT_BYTES_LEN);
        }

        tl.cur_collector = None;
    }
}

#[must_use]
pub struct SpanGuard {
    index: usize,

    _marker: PhantomData<*const ()>,
}

impl SpanGuard {
    pub(crate) fn new(event: u32) -> Option<Self> {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        if tl.cur_collector.is_none() || tl.enter_stack.is_empty() {
            return None;
        }

        let index = tl.spans.len();
        let parent_id = *tl.enter_stack.last().unwrap();

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

        tl.spans.push(Span {
            id,
            state: State::Local,
            related_id: parent_id,
            begin_cycles: minstant::now(),
            elapsed_cycles: 0,
            event,
        });

        Some(Self {
            index,
            _marker: PhantomData,
        })
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };
        tl.spans[self.index].elapsed_cycles =
            minstant::now().saturating_sub(tl.spans[self.index].begin_cycles);
        tl.enter_stack.pop();
    }
}

pub fn append_property<F, B>(f: F)
where
    B: AsRef<[u8]>,
    F: FnOnce() -> B,
{
    let trace = TRACE_LOCAL.with(|trace| trace.get());
    let tl = unsafe { &mut *trace };

    if tl.cur_collector.is_none() || tl.enter_stack.is_empty() {
        return;
    }

    let cur_span_id = *tl.enter_stack.last().unwrap();
    let payload = f();
    let payload = payload.as_ref();
    let payload_len = payload.len();

    tl.property_ids.push(cur_span_id);
    tl.property_lens.push(payload_len as u64);
    tl.property_payload.extend_from_slice(payload);
}
