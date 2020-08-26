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

pub fn start_trace<T: Into<u32>>(root_event: T) -> SpanGuard {
    let trace = TRACE_LOCAL.with(|trace| trace.get());
    let tl = unsafe { &mut *trace };

    let mut span = Span {
        id: tl.new_span_id(),
        state: State::Normal,
        relation_id: RelationId::Root,
        begin_cycles: 0,
        elapsed_cycles: 0,
        event: root_event.into(),
    };
    span.start();

    SpanGuard::enter(span, tl)
}

pub fn new_span<T: Into<u32>>(event: T) -> Option<SpanGuard> {
    let trace = TRACE_LOCAL.with(|trace| trace.get());
    let tl = unsafe { &mut *trace };

    if tl.enter_stack.is_empty() {
        return None;
    }

    let parent_id = *tl.enter_stack.last().unwrap();
    let mut span = Span {
        id: tl.new_span_id(),
        state: State::Normal,
        relation_id: RelationId::ChildOf(parent_id),
        begin_cycles: 0,
        elapsed_cycles: 0,
        event: event.into(),
    };
    span.start();

    Some(SpanGuard::enter(span, tl))
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

    pub fn submit_span(&mut self, span: Span) {
        if !self.enter_stack.is_empty() {
            self.span_set.spans.push(span);
        } else {
            (*SPAN_COLLECTOR).push(SpanSet::from_span(span));
        }
    }

    pub fn submit_span_set(&mut self) {
        if !self.span_set.is_empty() {
            (*SPAN_COLLECTOR).push(self.span_set.take());
        }
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
    #[inline]
    pub(crate) fn start(&mut self) {
        self.begin_cycles = minstant::now();
    }

    #[inline]
    pub(crate) fn stop(&mut self) {
        self.elapsed_cycles = minstant::now().saturating_sub(self.begin_cycles);
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
    span: Span,
    _marker: PhantomData<*const ()>,
}

impl SpanGuard {
    #[inline]
    pub(crate) fn enter(span: Span, tl: &mut TraceLocal) -> Self {
        tl.enter_stack.push(span.id);

        Self {
            span,
            _marker: PhantomData,
        }
    }

    pub fn span_id(&self) -> SpanId {
        self.span.id
    }

    pub fn detach(self) -> DetachGuard {
        let local = TRACE_LOCAL.with(|local| local.get());
        let tl = unsafe { &mut *local };

        self.exit(tl);

        DetachGuard::new(self.span)
    }

    #[inline]
    fn exit(&self, tl: &mut TraceLocal) {
        if let Some(idx) = tl
            .enter_stack
            .iter()
            .rev()
            .position(|span_id| *span_id == self.span.id)
        {
            tl.enter_stack.remove(tl.enter_stack.len() - idx - 1);
        }

        if tl.enter_stack.is_empty() {
            tl.submit_span_set();
        }
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        self.span.stop();
        tl.submit_span(self.span);
        self.exit(tl);
    }
}

#[must_use]
pub struct DetachGuard {
    span: Span,
}

impl DetachGuard {
    pub(crate) fn new(span: Span) -> Self {
        DetachGuard { span }
    }
}

impl Drop for DetachGuard {
    fn drop(&mut self) {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        self.span.stop();
        tl.submit_span(self.span);
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
