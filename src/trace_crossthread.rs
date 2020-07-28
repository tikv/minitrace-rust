// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

struct CrossthreadTraceInner {
    collector: std::sync::Arc<crate::collector::CollectorInner>,
    next_state: crate::State,
    next_related_id: u64,
    waiting_begin_cycles: u64,
}

pub struct CrossthreadTrace {
    inner: Option<CrossthreadTraceInner>,
}

pub struct LocalTraceGuard<'a> {
    _local: crate::trace_local::LocalTraceGuard,

    // `CrossthreadTrace` may be used to trace a `Future` task which
    // consists of a sequence of local-tracings.
    //
    // We can treat the end of current local-tracing as the creation of
    // the next local-tracing. By the moment that the next local-tracing
    // is started, the gap time is the wait time of the next local-tracing.
    //
    // Here is the mutable reference for this purpose.
    handle: &'a mut CrossthreadTraceInner,
}

impl Drop for LocalTraceGuard<'_> {
    fn drop(&mut self) {
        self.handle.waiting_begin_cycles = minstant::now();
    }
}

impl CrossthreadTrace {
    pub(crate) fn new() -> Self {
        let trace_local = crate::trace_local::TRACE_LOCAL.with(|trace_local| trace_local.get());
        let tl = unsafe { &mut *trace_local };

        if tl.cur_collector.is_none() || tl.enter_stack.is_empty() {
            return Self { inner: None };
        }

        let collector = tl.cur_collector.as_ref().unwrap().clone();
        let related_id = *tl.enter_stack.last().unwrap();
        Self {
            inner: Some(CrossthreadTraceInner {
                collector,
                next_state: crate::State::Spawning,
                next_related_id: related_id,
                waiting_begin_cycles: minstant::now(),
            }),
        }
    }

    pub fn trace_enable<T: Into<u32>>(&mut self, event: T) -> Option<LocalTraceGuard> {
        let event = event.into();
        if let Some(inner) = &mut self.inner {
            let now_cycles = minstant::now();
            if let Some((trace_guard, self_id)) = crate::trace_local::LocalTraceGuard::new(
                inner.collector.clone(),
                now_cycles,
                crate::LeadingSpan {
                    state: inner.next_state,
                    related_id: inner.next_related_id,
                    begin_cycles: inner.waiting_begin_cycles,
                    elapsed_cycles: now_cycles.saturating_sub(inner.waiting_begin_cycles),
                    event,
                },
            ) {
                // for next scheduling time
                inner.next_state = crate::State::Scheduling;
                inner.next_related_id = self_id;

                Some(LocalTraceGuard {
                    _local: trace_guard,
                    handle: inner,
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    pub(crate) fn new_root(collector: std::sync::Arc<crate::collector::CollectorInner>) -> Self {
        Self {
            inner: Some(CrossthreadTraceInner {
                collector,
                next_state: crate::State::Root,
                next_related_id: 0,
                waiting_begin_cycles: minstant::now(),
            }),
        }
    }
}
