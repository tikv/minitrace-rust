// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#[must_use]
#[inline]
pub fn trace_enable<T: Into<u32>>(
    event: T,
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
            event: event.into(),
        },
    )
    .unwrap(); // It's safe to unwrap because the collector always exists at present.

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
pub fn trace_crossthread() -> crate::trace_crossthread::CrossthreadTrace {
    crate::trace_crossthread::CrossthreadTrace::new()
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
