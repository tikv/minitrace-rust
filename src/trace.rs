// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

use crossbeam::channel::Sender;

use crate::collector::{Collector, SpanSet};

pub type SpanId = u32;

static GLOBAL_ID_COUNTER: AtomicU16 = AtomicU16::new(0);

thread_local! {
    pub static TRACE_LOCAL: std::cell::UnsafeCell<TraceLocal> = std::cell::UnsafeCell::new(TraceLocal {
        id_prefix: next_global_id_prefix(),
        id_suffix: 1,
        enter_stack: Vec::with_capacity(1024),
        span_set: SpanSet::with_capacity(),
        cur_collector: None,
    });
}

fn next_global_id_prefix() -> u16 {
    GLOBAL_ID_COUNTER.fetch_add(1, Ordering::AcqRel)
}

pub fn start_trace<T: Into<u32>>(trace_id: u64, root_event: T) -> (ScopeGuard, Collector) {
    let trace = TRACE_LOCAL.with(|trace| trace.get());
    let tl = unsafe { &mut *trace };

    if !tl.enter_stack.is_empty() {
        panic!("Trying to start trace within an existing trace scope.");
    }

    let span_inner = SpanGuardInner::enter(
        Span {
            id: tl.new_span_id(),
            state: State::Normal,
            parent_id: 0,
            begin_cycles: minstant::now(),
            elapsed_cycles: 0,
            event: root_event.into(),
        },
        tl,
    );

    let (tx, rx) = crossbeam::channel::unbounded();
    tl.cur_collector = Some(Arc::new(tx));

    (
        ScopeGuard { inner: span_inner },
        Collector::new(trace_id, rx),
    )
}

pub fn new_span<T: Into<u32>>(event: T) -> Option<SpanGuard> {
    let trace = TRACE_LOCAL.with(|trace| trace.get());
    let tl = unsafe { &mut *trace };

    if tl.enter_stack.is_empty() {
        return None;
    }

    let parent_id = *tl.enter_stack.last().unwrap();
    let span_inner = SpanGuardInner::enter(
        Span {
            id: tl.new_span_id(),
            state: State::Normal,
            parent_id,
            begin_cycles: minstant::now(),
            elapsed_cycles: 0,
            event: event.into(),
        },
        tl,
    );

    Some(SpanGuard { inner: span_inner })
}

/// The property is in bytes format, so it is not limited to be a key-value pair but
/// anything intended. However, the downside of flexibility is that manual encoding
/// and manual decoding need to consider.
pub fn new_property<B: AsRef<[u8]>>(p: B) {
    append_property(|| p);
}

/// `property` of closure version
pub fn new_property_with<F, B>(f: F)
where
    B: AsRef<[u8]>,
    F: FnOnce() -> B,
{
    append_property(f);
}

pub fn is_tracing() -> bool {
    let trace = TRACE_LOCAL.with(|trace| trace.get());
    let tl = unsafe { &mut *trace };

    !tl.enter_stack.is_empty()
}

pub struct TraceLocal {
    /// For id construction
    pub id_prefix: u16,
    pub id_suffix: u16,

    /// For parent-child relation construction. The last span, when exits, is
    /// responsible to submit the local span sets.
    pub enter_stack: Vec<SpanId>,
    pub span_set: SpanSet,

    pub cur_collector: Option<Arc<Sender<SpanSet>>>,
}

impl TraceLocal {
    #[inline]
    pub fn new_span_id(&mut self) -> SpanId {
        let id = ((self.id_prefix as u32) << 16) | self.id_suffix as u32;

        if self.id_suffix == std::u16::MAX {
            self.id_suffix = 1;
            self.id_prefix = next_global_id_prefix();
        } else {
            self.id_suffix += 1;
        }

        id
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum State {
    Normal,
    Pending,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Span {
    pub id: SpanId,
    pub state: State,
    pub parent_id: u32,
    pub begin_cycles: u64,
    pub elapsed_cycles: u64,
    pub event: u32,
}

#[must_use]
pub struct ScopeGuard {
    inner: SpanGuardInner,
}

impl Drop for ScopeGuard {
    #[inline]
    fn drop(&mut self) {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        self.inner.exit(tl);

        tl.cur_collector
            .as_ref()
            .unwrap()
            .try_send(tl.span_set.take())
            .ok();
        tl.cur_collector = None;
    }
}

#[must_use]
pub struct SpanGuard {
    pub(crate) inner: SpanGuardInner,
}

impl Drop for SpanGuard {
    #[inline]
    fn drop(&mut self) {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        self.inner.exit(tl);
    }
}

#[must_use]
pub struct SpanGuardInner {
    span_index: usize,
    _marker: PhantomData<*const ()>,
}

impl SpanGuardInner {
    #[inline]
    pub fn enter(span: Span, tl: &mut TraceLocal) -> Self {
        tl.enter_stack.push(span.id);
        tl.span_set.spans.push(span);

        Self {
            span_index: tl.span_set.spans.len() - 1,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn exit(&mut self, tl: &mut TraceLocal) -> u64 {
        debug_assert_eq!(
            *tl.enter_stack.last().unwrap(),
            tl.span_set.spans[self.span_index].id
        );

        tl.enter_stack.pop();

        let now_cycle = minstant::now();
        let span = &mut tl.span_set.spans[self.span_index];
        span.elapsed_cycles = now_cycle.wrapping_sub(span.begin_cycles);

        now_cycle
    }
}

fn append_property<F, B>(f: F)
where
    B: AsRef<[u8]>,
    F: FnOnce() -> B,
{
    let trace = TRACE_LOCAL.with(|trace| trace.get());
    let tl = unsafe { &mut *trace };

    if tl.enter_stack.is_empty() {
        return;
    }

    let cur_span_id = *tl.enter_stack.last().unwrap();
    let payload = f();
    let payload = payload.as_ref();
    let payload_len = payload.len();

    tl.span_set.properties.span_ids.push(cur_span_id);
    tl.span_set
        .properties
        .property_lens
        .push(payload_len as u64);
    tl.span_set.properties.payload.extend_from_slice(payload);
}
