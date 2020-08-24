// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![feature(shrink_to)]

pub mod future;
pub mod thread;

mod collector;
mod trace;
mod utils;

pub use collector::Collector;
pub use trace::{ScopeGuard, SpanGuard};

pub use minitrace_macro::{trace, trace_async};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct TraceResult {
    /// The start time of the whole tracing process that is the time
    /// when calling `start_trace`
    pub start_time_ns: u64,

    /// The elapsed of the whole tracing process that is the time diff
    /// from calling `start_trace` to calling `collect`
    pub elapsed_ns: u64,

    /// For conversion of cycles -> ns
    pub cycles_per_second: u64,

    /// Span collection
    pub spans: Vec<Span>,

    /// Properties
    pub properties: Properties,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Span {
    pub id: u64,
    pub state: State,
    pub related_id: u64,
    pub begin_cycles: u64,
    pub elapsed_cycles: u64,
    pub event: u32,
}

/// `State` represents the attributes of a span and how the span relates to
/// another span with `related_id`.
///
/// ## Root
/// ```text
/// ------------------------------- TIME LINE -------------------------------------->
///
/// | <- enable trace
/// +------------------------------------+-------------------------------+
/// | state: Root, related_id: 0, id: 88 | state: Settle, related_id: 88 |
/// +------------------------------------+-------------------------------+
/// ```
///
/// ## Local
/// ```text
/// ------------------------------- TIME LINE -------------------------------------->
///
/// +--------------------------------------+
/// |          A span with id 42           |
/// +--------------------------------------+
///       | <- new_span()
///       +------------------------------+
///       | state: Local, related_id: 42 |
///       +------------------------------+
/// ```
///
/// ## Spawning & Settle
/// ```text
/// ------------------------------- TIME LINE -------------------------------------->
///
/// +--------------------------------------+
/// |          A span with id 42           |
/// +--------------------------------------+
///       | let handle = trace_crossthread();
///       | <- thread::spawn()
///       +-----------------------------------------+-------------------------------+
///       | state: Spawning, related_id: 42, id: 77 | state: Settle, related_id: 77 |
///       +-----------------------------------------+-------------------------------+
///                                                 | <- handle.start_trace()
/// ```
///
/// ## Scheduling & Settle
/// ```text
/// ------------------------------- TIME LINE -------------------------------------->
///
/// +--------------------------------------+
/// |          A span with id 42           |
/// +--------------------------------------+
///   | <- runtime::spawn(future)
///   +-----------------------------------------+-------------------------------+
///   | state: Spawning, related_id: 42, id: 77 | state: Settle, related_id: 77 |
///   +-----------------------------------------+-------------------------------+
///                                             | <- future.poll()              | <- poll() return                          | <- future.poll()
///                                                                             +-------------------------------------------+-------------------------------+
///                                                                             | state: Scheduling, related_id: 77, id: 23 | state: Settle, related_id: 23 |
///                                                                             +-------------------------------------------+-------------------------------+
/// ```
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum State {
    Root,
    Local,
    Spawning,
    Scheduling,
    Settle,
}

/// Properties can used to attach some information about tracing context
/// to current span, e.g. host of the request, CPU usage.
///
/// Usage:
/// ```
/// # let event_id = 1u32;
/// let _guard = minitrace::new_span(event_id);
/// minitrace::property(b"host:127.0.0.1");
/// minitrace::property(b"cpu_usage:42%");
/// ```
///
/// Every property will relate to a span. Logically properties are a sequence
/// of (span id, property) pairs:
/// ```text
/// span id -> property
/// 10      -> b"123"
/// 10      -> b"!@$#$%"
/// 12      -> b"abcd"
/// 14      -> b"xyz"
/// ```
///
/// and will be stored into `Properties` struct as:
/// ```text
/// span_ids: [10, 10, 12, 14]
/// property_lens: [3, 6, 4, 3]
/// payload: b"123!@$#$%abcdxyz"
/// ```
#[derive(Debug, Clone)]
pub struct Properties {
    pub span_ids: Vec<u64>,
    pub property_lens: Vec<u64>,
    pub payload: Vec<u8>,
}

pub fn start_trace<T: Into<u32>>(event: T) -> Option<(ScopeGuard, Collector)> {
    let now_cycles = minstant::now();
    let collector = Collector::new();

    let event = event.into();
    let (scope_guard, _) = ScopeGuard::new(
        collector.inner.clone(),
        now_cycles,
        crate::trace::LeadingSpan {
            state: State::Root,
            related_id: 0,
            begin_cycles: now_cycles,
            elapsed_cycles: 0,
            event,
        },
        event,
    )?;

    Some((scope_guard, collector))
}

#[inline]
pub fn new_span<T: Into<u32>>(event: T) -> Option<crate::trace::SpanGuard> {
    crate::trace::SpanGuard::new(event.into())
}

/// The property is in bytes format, so it is not limited to be a key-value pair but
/// anything intended. However, the downside of flexibility is that manual encoding
/// and manual decoding need to consider.
#[inline]
pub fn new_property<B: AsRef<[u8]>>(p: B) {
    crate::trace::append_property(|| p);
}

/// `property` of closure version
#[inline]
pub fn new_property_with<F, B>(f: F)
where
    B: AsRef<[u8]>,
    F: FnOnce() -> B,
{
    crate::trace::append_property(f);
}
