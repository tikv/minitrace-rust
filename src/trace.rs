// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::collector::{SpanSet, SPAN_COLLECTOR};

pub type SpanId = u64;

static GLOBAL_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

thread_local! {
    pub static TRACE_LOCAL: std::cell::UnsafeCell<TraceLocal> = std::cell::UnsafeCell::new(TraceLocal {
        id_prefix: next_global_id_prefix(),
        id_suffix: 0,
        enter_stack: Vec::with_capacity(1024),
        span_set: SpanSet::with_capacity(),
    });
}

fn next_global_id_prefix() -> u32 {
    GLOBAL_ID_COUNTER.fetch_add(1, Ordering::AcqRel)
}

pub fn start_trace<T: Into<u32>>(root_event: T) -> Option<ScopeGuard> {
    let trace = TRACE_LOCAL.with(|trace| trace.get());
    let tl = unsafe { &mut *trace };

    if !tl.enter_stack.is_empty() {
        return None;
    }

    unsafe {
        Some(ScopeGuard::enter(tl, |span| {
            span.state = State::Normal;
            span.relation_id = RelationId::Root;
            span.event = root_event.into();
        }))
    }
}

pub fn new_span<T: Into<u32>>(event: T) -> Option<SpanGuard> {
    let trace = TRACE_LOCAL.with(|trace| trace.get());
    let tl = unsafe { &mut *trace };

    if tl.enter_stack.is_empty() {
        return None;
    }

    let parent_id = *tl.enter_stack.last().unwrap();
    unsafe {
        Some(SpanGuard::enter(tl, |span| {
            span.state = State::Normal;
            span.relation_id = RelationId::ChildOf(parent_id);
            span.event = event.into();
        }))
    }
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

pub struct TraceLocal {
    /// For id construction
    pub id_prefix: u32,
    pub id_suffix: u32,

    /// For parent-child relation construction. The last span, when exits, is
    /// responsible to submit the local span sets.
    pub enter_stack: Vec<SpanId>,
    pub span_set: SpanSet,
}

impl TraceLocal {
    #[inline(always)]
    pub fn new_span_id(&mut self) -> SpanId {
        let id = ((self.id_prefix as u64) << 32) | self.id_suffix as u64;

        if self.id_suffix == std::u32::MAX {
            self.id_suffix = 0;
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
    pub relation_id: RelationId,
    pub begin_cycles: u64,
    pub elapsed_cycles: u64,
    pub event: u32,
}

impl Span {
    #[inline(always)]
    pub(crate) fn start(&mut self) {
        self.begin_cycles = unsafe { core::arch::x86_64::_rdtsc() };
    }

    #[inline(always)]
    pub(crate) fn stop(&mut self) {
        self.elapsed_cycles =
            unsafe { core::arch::x86_64::_rdtsc() }.saturating_sub(self.begin_cycles);
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RelationId {
    Root,
    ChildOf(SpanId),
    FollowFrom(SpanId),
}

#[must_use]
pub struct SpanGuard {
    span_index: usize,
    _marker: PhantomData<*const ()>,
}

impl SpanGuard {
    #[inline(always)]
    pub(crate) unsafe fn enter<F>(tl: &mut TraceLocal, init: F) -> Self
    where
        F: FnOnce(&mut Span),
    {
        let id = tl.new_span_id();
        tl.enter_stack.push(id);

        let spans = &mut tl.span_set.spans;
        spans.reserve(1);
        let span_index = spans.len();
        spans.set_len(span_index + 1);

        let span = spans.get_unchecked_mut(span_index);
        span.id = id;
        span.begin_cycles = unsafe { core::arch::x86_64::_rdtsc() };
        init(span);

        Self {
            span_index,
            _marker: PhantomData,
        }
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        tl.enter_stack.pop().unwrap();
        unsafe {
            tl.span_set.spans.get_unchecked_mut(self.span_index).stop();
        }
    }
}

#[must_use]
pub struct ScopeGuard {
    span_index: usize,
    _marker: PhantomData<*const ()>,
}

impl ScopeGuard {
    #[inline(always)]
    pub(crate) unsafe fn enter<F>(tl: &mut TraceLocal, init: F) -> Self
    where
        F: FnOnce(&mut Span),
    {
        let id = tl.new_span_id();
        tl.enter_stack.push(id);

        let spans = &mut tl.span_set.spans;
        spans.reserve(1);
        let span_index = spans.len();
        spans.set_len(span_index + 1);

        let span = spans.get_unchecked_mut(span_index);
        span.id = id;
        span.begin_cycles = unsafe { core::arch::x86_64::_rdtsc() };
        init(span);

        Self {
            span_index,
            _marker: PhantomData,
        }
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        tl.enter_stack.pop().unwrap();
        unsafe {
            tl.span_set.spans.get_unchecked_mut(self.span_index).stop();
        }

        (*SPAN_COLLECTOR).push(tl.span_set.take());
    }
}

pub fn append_property<F, B>(f: F)
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
