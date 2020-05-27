#[must_use]
#[inline]
pub fn trace_enable(
    event: u32,
) -> (
    crate::trace_local::LocalTraceGuard,
    crate::collector::Collector,
) {
    let collector = std::sync::Arc::new(crate::collector::CollectorInner {
        queue: crossbeam::queue::SegQueue::new(),
        closed: std::sync::atomic::AtomicBool::new(false),
    });

    let (trace_guard, _) = crate::trace_local::LocalTraceGuard::new(
        collector.clone(),
        event,
        crate::Link::Root,
        crate::time::real_time_ns(),
    )
    .unwrap();

    let collector = crate::collector::Collector { inner: collector };

    (trace_guard, collector)
}

#[must_use]
#[inline]
pub fn trace_may_enable(
    enable: bool,
    event: u32,
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
pub fn new_span(event: u32) -> Option<crate::trace_local::SpanGuard> {
    crate::trace_local::SpanGuard::new(event)
}

#[must_use]
#[inline]
pub fn trace_crossthread(event: u32) -> crate::trace_crossthread::CrossthreadTrace {
    crate::trace_crossthread::CrossthreadTrace::new(event)
}
