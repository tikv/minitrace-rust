// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

struct CrossthreadTraceInner {
    collector: std::sync::Arc<crate::collector::CollectorInner>,
    state: crate::State,
    related_id: u64,
    begin_cycles: u64,
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
    begin_cycles: &'a mut u64,
}

impl Drop for LocalTraceGuard<'_> {
    fn drop(&mut self) {
        *self.begin_cycles = minstant::now();
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
                state: crate::State::Spawning,
                related_id,
                begin_cycles: minstant::now(),
            }),
        }
    }

    pub fn trace_enable<T: Into<u32>>(&mut self, event: T) -> Option<LocalTraceGuard> {
        let event = event.into();
        if let Some(inner) = &mut self.inner {
            if let Some((trace_guard, self_id)) = crate::trace_local::LocalTraceGuard::new(
                inner.collector.clone(),
                event,
                if inner.state == crate::State::Root {
                    None
                } else {
                    Some(crate::LeadingSpanArg {
                        state: inner.state,
                        related_id: inner.related_id,
                        begin_cycles: inner.begin_cycles,
                        elapsed_cycles: minstant::now().saturating_sub(inner.begin_cycles),
                        event,
                    })
                },
            ) {
                // for next scheduling time
                inner.state = crate::State::Scheduling;
                inner.related_id = self_id;

                Some(LocalTraceGuard {
                    _local: trace_guard,
                    begin_cycles: &mut inner.begin_cycles,
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
                state: crate::State::Root,
                begin_cycles: 0,
                related_id: 0,
            }),
        }
    }
}
