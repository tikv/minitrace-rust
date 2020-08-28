// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use either::Either;

use crate::collector::SPAN_COLLECTOR;
use crate::trace::*;

/// Bind the current tracing context to another executing context (e.g. a closure).
///
/// ```
/// # use minitrace::thread::new_async_scope;
/// # use std::thread;
/// #
/// let mut handle = new_async_scope();
/// thread::spawn(move || {
///     let _g = handle.start_trace(0, 0u32);
/// });
/// ```
#[inline]
pub fn new_async_handle() -> AsyncHandle {
    let trace = TRACE_LOCAL.with(|trace| trace.get());
    let tl = unsafe { &mut *trace };

    if tl.enter_stack.is_empty() {
        return AsyncHandle { inner: None };
    }

    let parent_id = *tl.enter_stack.last().unwrap();
    let inner = AsyncHandleInner {
        trace_id: tl.last_trace_id,
        parent_id,
        begin_cycles: minstant::now(),
    };

    AsyncHandle { inner: Some(inner) }
}

struct AsyncHandleInner {
    trace_id: u32,
    parent_id: u32,
    begin_cycles: u64,
}

#[must_use]
pub struct AsyncHandle {
    /// None indicates that tracing is not enabled
    inner: Option<AsyncHandleInner>,
}

impl AsyncHandle {
    pub fn start_trace<T: Into<u32>>(
        &mut self,
        event: T,
    ) -> Option<Either<AsyncScopeGuard<'_>, SpanGuard>> {
        if self.inner.is_none() {
            return None;
        }

        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        let event = event.into();
        if tl.enter_stack.is_empty() {
            Some(Either::Left(self.new_scope(event, tl)))
        } else {
            Some(Either::Right(self.new_span(event, tl)))
        }
    }

    fn new_scope(&mut self, event: u32, tl: &mut TraceLocal) -> AsyncScopeGuard<'_> {
        let inner = self.inner.as_mut().unwrap();

        let pending_id = tl.new_span_id();
        let pending_span = Span {
            id: pending_id,
            state: State::Pending,
            parent_id: inner.parent_id,
            begin_cycles: inner.begin_cycles,
            elapsed_cycles: minstant::now().wrapping_sub(inner.begin_cycles),
            event,
        };
        tl.span_set.spans.push(pending_span);

        let span_id = tl.new_span_id();
        let span_inner = SpanGuardInner::enter(
            Span {
                id: span_id,
                state: State::Normal,
                parent_id: pending_id,
                begin_cycles: minstant::now(),
                elapsed_cycles: 0,
                event: event.into(),
            },
            tl,
        );
        inner.parent_id = span_id;
        tl.last_trace_id = inner.trace_id;

        AsyncScopeGuard {
            inner: span_inner,
            handle: self,
        }
    }

    fn new_span(&mut self, event: u32, tl: &mut TraceLocal) -> SpanGuard {
        let inner = self.inner.as_mut().unwrap();

        let parent_id = *tl.enter_stack.last().unwrap();
        let span_inner = SpanGuardInner::enter(
            Span {
                id: tl.new_span_id(),
                state: State::Normal,
                parent_id,
                begin_cycles: if inner.begin_cycles != 0 {
                    inner.begin_cycles
                } else {
                    minstant::now()
                },
                elapsed_cycles: 0,
                event: event.into(),
            },
            tl,
        );
        inner.begin_cycles = 0;

        SpanGuard { inner: span_inner }
    }
}

pub struct AsyncScopeGuard<'a> {
    inner: SpanGuardInner,
    handle: &'a mut AsyncHandle,
}

impl<'a> Drop for AsyncScopeGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        let now_cycle = self.inner.exit(tl);
        self.handle.inner.as_mut().unwrap().begin_cycles = now_cycle;

        (*SPAN_COLLECTOR).push((tl.last_trace_id, tl.span_set.take()));
    }
}
