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
///     let _g = handle.start_trace(0u32);
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
        parent_id,
        begin_cycles: minstant::now(),
    };

    AsyncHandle { inner: Some(inner) }
}

struct AsyncHandleInner {
    parent_id: u64,
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
        let inner = self.inner.as_mut()?;

        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        let event = event.into();
        if tl.enter_stack.is_empty() {
            let pending_span = Span {
                id: tl.new_span_id(),
                state: State::Pending,
                parent_id: inner.parent_id,
                begin_cycles: inner.begin_cycles,
                elapsed_cycles: minstant::now().saturating_sub(inner.begin_cycles),
                event,
            };
            tl.span_set.spans.push(pending_span);

            let span_inner = SpanGuardInner::enter(
                Span {
                    id: tl.new_span_id(),
                    state: State::Normal,
                    parent_id: inner.parent_id,
                    begin_cycles: minstant::now(),
                    elapsed_cycles: 0,
                    event: event.into(),
                },
                tl,
            );

            Some(Either::Left(AsyncScopeGuard {
                inner: span_inner,
                handle: self,
            }))
        } else {
            let span_inner = SpanGuardInner::enter(
                Span {
                    id: tl.new_span_id(),
                    state: State::Normal,
                    parent_id: inner.parent_id,
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

            Some(Either::Right(SpanGuard { inner: span_inner }))
        }
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

        (*SPAN_COLLECTOR).push(tl.span_set.take());
    }
}
