// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::Arc;

use crossbeam::channel::Sender;

use crate::collector::SpanSet;
use crate::trace::*;

/// Bind the current tracing context to another executing context.
///
/// ```
/// # use minitrace::thread::new_async_handle;
/// # use std::thread;
/// #
/// let mut handle = new_async_handle();
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
        collector: tl.cur_collector.clone().unwrap(),
        next_pending_parent_id: parent_id,
        begin_cycles: minstant::now(),
    };

    AsyncHandle { inner: Some(inner) }
}

struct AsyncHandleInner {
    collector: Arc<Sender<SpanSet>>,
    next_pending_parent_id: u32,
    begin_cycles: u64,
}

#[must_use]
pub struct AsyncHandle {
    /// None indicates that tracing is not enabled
    inner: Option<AsyncHandleInner>,
}

impl AsyncHandle {
    pub fn start_trace<T: Into<u32>>(&mut self, event: T) -> Option<AsyncGuard<'_>> {
        let inner = self.inner.as_mut()?;

        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        let event = event.into();
        if tl.enter_stack.is_empty() {
            Some(AsyncGuard::AsyncScopeGuard(Self::new_scope(
                inner, event, tl,
            )))
        } else {
            Some(AsyncGuard::SpanGuard(Self::new_span(inner, event, tl)))
        }
    }

    #[inline]
    fn new_scope<'a>(
        handle_inner: &'a mut AsyncHandleInner,
        event: u32,
        tl: &mut TraceLocal,
    ) -> AsyncScopeGuard<'a> {
        let pending_id = tl.new_span_id();
        let pending_span = Span {
            id: pending_id,
            state: State::Pending,
            parent_id: handle_inner.next_pending_parent_id,
            begin_cycles: handle_inner.begin_cycles,
            elapsed_cycles: minstant::now().wrapping_sub(handle_inner.begin_cycles),
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
                event,
            },
            tl,
        );
        handle_inner.next_pending_parent_id = span_id;

        tl.cur_collector = Some(handle_inner.collector.clone());

        AsyncScopeGuard {
            span_inner,
            handle_inner,
        }
    }

    #[inline]
    fn new_span(handle_inner: &mut AsyncHandleInner, event: u32, tl: &mut TraceLocal) -> SpanGuard {
        let parent_id = *tl.enter_stack.last().unwrap();
        let span_inner = SpanGuardInner::enter(
            Span {
                id: tl.new_span_id(),
                state: State::Normal,
                parent_id,
                begin_cycles: if handle_inner.begin_cycles != 0 {
                    handle_inner.begin_cycles
                } else {
                    minstant::now()
                },
                elapsed_cycles: 0,
                event,
            },
            tl,
        );
        handle_inner.begin_cycles = 0;

        SpanGuard { inner: span_inner }
    }
}

pub enum AsyncGuard<'a> {
    AsyncScopeGuard(AsyncScopeGuard<'a>),
    SpanGuard(SpanGuard),
}

pub struct AsyncScopeGuard<'a> {
    span_inner: SpanGuardInner,
    handle_inner: &'a mut AsyncHandleInner,
}

impl<'a> Drop for AsyncScopeGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        let trace = TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        let now_cycle = self.span_inner.exit(tl);
        self.handle_inner.begin_cycles = now_cycle;
        self.handle_inner.collector.send(tl.span_set.take()).ok();

        tl.cur_collector = None;
    }
}
