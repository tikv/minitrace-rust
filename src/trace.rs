// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#[must_use]
#[inline]
pub fn trace_enable<T: Into<u32>>(
    event: T,
) -> (
    crate::trace_local::LocalTraceGuard,
    crate::collector::Collector,
) {
    let collector = std::sync::Arc::new(crate::collector::CollectorInner {
        queue: crossbeam::queue::SegQueue::new(),
        closed: std::sync::atomic::AtomicBool::new(false),
    });

    let now = crate::time::real_time_ns();
    let (trace_guard, _) = crate::trace_local::LocalTraceGuard::new(
        collector.clone(),
        event,
        crate::Link::Root,
        now,
        now,
    )
    .unwrap();

    let collector = crate::collector::Collector { inner: collector };

    (trace_guard, collector)
}

#[must_use]
#[inline]
pub fn trace_may_enable<T: Into<u32>>(
    enable: bool,
    event: T,
) -> (
    Option<crate::trace_local::LocalTraceGuard>,
    Option<crate::collector::Collector>,
) {
    if enable {
        let (guard, collector) = trace_enable(event);
        (Some(guard), Some(collector))
    } else {
        (None, None)
    }
}

#[must_use]
#[inline]
pub fn new_span<T: Into<u32>>(event: T) -> Option<crate::trace_local::SpanGuard> {
    crate::trace_local::SpanGuard::new(event.into())
}

#[must_use]
#[inline]
pub fn trace_crossthread<T: Into<u32>>(event: T) -> crate::trace_crossthread::CrossthreadTrace {
    crate::trace_crossthread::CrossthreadTrace::new(event.into())
}
