// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::CollectorInner;
use crate::{ScopeGuard, State};

/// Bind the current tracing context to another executing context (e.g. a closure).
///
/// ```no_run
/// # use minitrace::new_async_scope;
/// # use std::thread;
/// #
/// let handle = new_async_scope();
/// thread::spawn(move || {
///     let mut handle = handle;
///     let _g = handle.start_trace(EVENT);
/// });
/// ```
#[inline]
pub fn new_async_scope() -> AsyncScopeHandle {
    crate::thread::AsyncScopeHandle::new()
}

struct AsyncScopeInner {
    collector: std::sync::Arc<CollectorInner>,
    next_suspending_state: State,
    next_related_id: u64,
    suspending_begin_cycles: u64,
}

#[must_use]
pub struct AsyncScopeHandle {
    inner: Option<AsyncScopeInner>,
}

pub struct AsyncScopeGuard<'a> {
    _local: ScopeGuard,

    // `AsyncScopeHandle` may be used to trace a `Future` task which
    // consists of a sequence of local-tracings.
    //
    // We can treat the end of current local-tracing as the creation of
    // the next local-tracing. By the moment that the next local-tracing
    // is started, the gap time is the wait time of the next local-tracing.
    //
    // Here is the mutable reference for this purpose.
    handle: &'a mut AsyncScopeInner,
}

impl Drop for AsyncScopeGuard<'_> {
    fn drop(&mut self) {
        self.handle.suspending_begin_cycles = minstant::now();
    }
}

impl AsyncScopeHandle {
    fn new() -> Self {
        let trace = crate::trace::TRACE_LOCAL.with(|trace| trace.get());
        let tl = unsafe { &mut *trace };

        if tl.cur_collector.is_none() || tl.enter_stack.is_empty() {
            return Self { inner: None };
        }

        let collector = tl.cur_collector.as_ref().unwrap().clone();
        let related_id = *tl.enter_stack.last().unwrap();
        Self {
            inner: Some(AsyncScopeInner {
                collector,
                next_suspending_state: State::Spawning,
                next_related_id: related_id,
                suspending_begin_cycles: minstant::now(),
            }),
        }
    }

    pub(crate) fn new_root(collector: std::sync::Arc<crate::collector::CollectorInner>) -> Self {
        let now_cycles = minstant::now();
        Self {
            inner: Some(AsyncScopeInner {
                collector,
                next_suspending_state: State::Root,
                next_related_id: 0,
                suspending_begin_cycles: now_cycles,
            }),
        }
    }

    pub fn start_trace<E: Into<u32>>(&mut self, event: E) -> Option<AsyncScopeGuard> {
        if let Some(inner) = &mut self.inner {
            let event = event.into();

            let now_cycles = minstant::now();
            if let Some((local_guard, self_id)) = crate::trace::ScopeGuard::new(
                inner.collector.clone(),
                now_cycles,
                crate::trace::LeadingSpan {
                    // At this restoring time, fill this leading span with the
                    // related id, begin cycles and ...
                    state: inner.next_suspending_state,
                    related_id: inner.next_related_id,
                    begin_cycles: inner.suspending_begin_cycles,
                    // ... other fields calculating via them.
                    elapsed_cycles: now_cycles.saturating_sub(inner.suspending_begin_cycles),
                    event,
                },
                event,
            ) {
                // Reserve these for the next suspending process
                inner.next_related_id = self_id;

                // Obviously, the begin cycles of the next suspending is impossible to predict, and it should
                // be recorded when `local_guard` is dropping. Here `AsyncScopeGuard` is for this purpose.
                // See `impl Drop for AsyncScopeGuard`.
                Some(AsyncScopeGuard {
                    _local: local_guard,
                    handle: inner,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}
