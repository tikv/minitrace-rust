// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#[inline]
pub fn trace_enable<T: Into<u32>>(
    event: T,
) -> (
    crate::trace_local::LocalTraceGuard,
    crate::collector::Collector,
) {
    let event = event.into();
    trace_enable_fine(event, event)
}

#[inline]
pub fn trace_enable_fine<E1: Into<u32>, E2: Into<u32>>(
    pending_event: E1,
    settle_event: E2,
) -> (
    crate::trace_local::LocalTraceGuard,
    crate::collector::Collector,
) {
    let now_cycles = minstant::now();
    let collector = crate::collector::Collector::new(crate::time::real_time_ns());

    let (trace_guard, _) = crate::trace_local::LocalTraceGuard::new(
        collector.inner.clone(),
        now_cycles,
        crate::LeadingSpan {
            state: crate::State::Root,
            related_id: 0,
            begin_cycles: now_cycles,
            elapsed_cycles: 0,
            event: pending_event.into(),
        },
        settle_event.into(),
    )
    .unwrap(); // It's safe to unwrap because the collector always exists at present.

    (trace_guard, collector)
}

#[inline]
pub fn trace_may_enable<T: Into<u32>>(
    enable: bool,
    event: T,
) -> (
    Option<crate::trace_local::LocalTraceGuard>,
    Option<crate::collector::Collector>,
) {
    let event = event.into();
    trace_may_enable_fine(enable, event, event)
}

#[inline]
pub fn trace_may_enable_fine<E1: Into<u32>, E2: Into<u32>>(
    enable: bool,
    pending_event: E1,
    settle_event: E2,
) -> (
    Option<crate::trace_local::LocalTraceGuard>,
    Option<crate::collector::Collector>,
) {
    if enable {
        let (guard, collector) = trace_enable_fine(pending_event, settle_event);
        (Some(guard), Some(collector))
    } else {
        (None, None)
    }
}

#[inline]
pub fn new_span<T: Into<u32>>(event: T) -> Option<crate::trace_local::SpanGuard> {
    crate::trace_local::SpanGuard::new(event.into())
}

/// Bind the current tracing context to another executing context (e.g. a closure).
///
/// ```no_run
/// # use minitrace::trace_binder;
/// # use std::thread;
/// #
/// let handle = trace_binder();
/// thread::spawn(move || {
///     let mut handle = handle;
///     let _g = handle.trace_enable(EVENT);
/// });
/// ```
#[inline]
pub fn trace_binder() -> crate::trace_async::TraceHandle {
    crate::trace_async::TraceHandle::new(None)
}

#[inline]
pub fn trace_binder_fine<E: Into<u32>>(pending_event: E) -> crate::trace_async::TraceHandle {
    crate::trace_async::TraceHandle::new(Some(pending_event.into()))
}

/// The property is in bytes format, so it is not limited to be a key-value pair but
/// anything intended. However, the downside of flexibility is that manual encoding
/// and manual decoding need to consider.
#[inline]
pub fn property<B: AsRef<[u8]>>(p: B) {
    crate::trace_local::append_property(|| p);
}

/// `property` of closure version
#[inline]
pub fn property_closure<F, B>(f: F)
where
    B: AsRef<[u8]>,
    F: FnOnce() -> B,
{
    crate::trace_local::append_property(f);
}
