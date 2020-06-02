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

    pub fn trace_enable(&mut self) -> Option<crate::trace_local::LocalTraceGuard> {
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
                Some(trace_guard)
            } else {
                self.inner = None;
                None
            }
        } else {
            None
        }
    }
}
