// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::trace::{RelationId, Span, SpanGuard, State, TRACE_LOCAL};

/// Bind the current tracing context to another executing context (e.g. a closure).
///
/// ```
/// # use minitrace::thread::new_async_scope;
/// # use std::thread;
/// #
/// let mut handle = new_async_scope();
/// thread::spawn(move || {
///     let _g = handle.start_trace(0u32);
/// });
/// ```
#[inline]
pub fn new_async_span() -> AsyncGuard {
    AsyncGuard::start(false)
}

#[must_use]
pub struct AsyncGuard {
    /// If None, it indicates tracing is not enabled
    pub(crate) pending_span: Option<Span>,
}

impl AsyncGuard {
    pub(crate) fn new_empty() -> Self {
        AsyncGuard { pending_span: None }
    }

    pub(crate) fn start(follow_from_parent: bool) -> Self {
        let trace = crate::trace::TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        if tl.enter_stack.is_empty() {
            return AsyncGuard::new_empty();
        }

        let parent_id = *tl.enter_stack.last().unwrap();
        let relation_id = if follow_from_parent {
            RelationId::FollowFrom(parent_id)
        } else {
            RelationId::ChildOf(parent_id)
        };

        let mut pending_span = Span {
            id: tl.new_span_id(),
            state: State::Pending,
            relation_id,
            begin_cycles: 0,
            elapsed_cycles: 0,
            event: 0,
        };
        pending_span.start();

        Self {
            pending_span: Some(pending_span),
        }
    }

    pub fn ready<E: Into<u32>>(self, event: E) -> Option<SpanGuard> {
        let mut pending_span = self.pending_span?;

        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        let event = event.into();
        pending_span.event = event;

        let mut span = Span {
            id: tl.new_span_id(),
            state: State::Normal,
            relation_id: RelationId::FollowFrom(pending_span.id),
            begin_cycles: 0,
            elapsed_cycles: 0,
            event,
        };
        span.start();

        let guard = SpanGuard::enter(span, tl);

        // Submit pending_span within the scope of the new span,
        // so as to reduce a SpanSet allocation.
        tl.submit_span(pending_span);

        Some(guard)
    }
}
