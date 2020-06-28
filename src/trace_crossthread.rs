// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

struct CrossthreadTraceInner {
    collector: std::sync::Arc<crate::collector::CollectorInner>,
    link: crate::Link,
    event: u32,
    create_time_ns: u64,
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
    create_time_ns: &'a mut u64,
}

impl Drop for LocalTraceGuard<'_> {
    fn drop(&mut self) {
        *self.create_time_ns = crate::time::real_time_ns();
    }
}

impl CrossthreadTrace {
    pub(crate) fn new(event: u32) -> Self {
        let trace_local = crate::trace_local::TRACE_LOCAL.with(|trace_local| trace_local.get());
        let tl = unsafe { &mut *trace_local };

        if tl.cur_collector.is_none() || tl.enter_stack.is_empty() {
            return Self { inner: None };
        }

        let collector = tl.cur_collector.as_ref().unwrap().clone();
        let link = crate::Link::Parent {
            id: *tl.enter_stack.last().unwrap(),
        };
        Self {
            inner: Some(CrossthreadTraceInner {
                collector,
                link,
                event,
                create_time_ns: crate::time::real_time_ns(),
            }),
        }
    }

    pub fn trace_enable(&mut self) -> Option<LocalTraceGuard> {
        if let Some(inner) = &mut self.inner {
            let now = crate::time::real_time_ns();
            if let Some((trace_guard, id)) = crate::trace_local::LocalTraceGuard::new(
                inner.collector.clone(),
                inner.event,
                inner.link,
                inner.create_time_ns,
                now,
            ) {
                inner.link = crate::Link::Continue { id };
                Some(LocalTraceGuard {
                    _local: trace_guard,
                    create_time_ns: &mut inner.create_time_ns,
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    pub(crate) fn new_root(
        event: u32,
        collector: std::sync::Arc<crate::collector::CollectorInner>,
    ) -> Self {
        Self {
            inner: Some(CrossthreadTraceInner {
                collector,
                link: crate::Link::Root,
                event,
                create_time_ns: crate::time::real_time_ns(),
            }),
        }
    }
}
