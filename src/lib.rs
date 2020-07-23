// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![feature(negative_impls)]
#![feature(shrink_to)]

pub(crate) mod collector;
pub mod future;
pub mod prelude;
pub(crate) mod time;
pub(crate) mod trace;
pub(crate) mod trace_crossthread;
pub(crate) mod trace_local;

pub use collector::*;
pub use trace::*;
pub use trace_crossthread::*;
pub use trace_local::*;

#[cfg(test)]
mod tests;

pub use minitrace_attribute::{trace, trace_async};

#[derive(Debug, Clone)]
pub struct TraceDetails {
    /// The start time of the whole tracing process that is the time
    /// when calling `trace_enable`
    pub start_time_ns: u64,

    /// The elapsed of the whole tracing process that is the time diff
    /// from calling `trace_enable` to calling `collect`
    pub elapsed_ns: u64,

    /// For conversion of cycles -> ns
    pub cycles_per_second: u64,

    /// Spanset collection
    pub span_sets: Vec<SpanSet>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Span {
    pub id: u64,
    pub link: Link,
    // TODO: add cargo feature to allow altering to ns
    pub begin_cycles: u64,
    pub elapsed_cycles: u64,
    pub event: u32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Link {
    Root,
    Parent { id: u64 },
    Continue { id: u64 },
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
/// 
/// ```
///
/// Every property will relate to a span. Logically properties are a sequence
/// of (span id, property) pairs:
/// ```ignore
/// span id -> property
/// 10      -> b"123"
/// 10      -> b"!@$#$%"
/// 12      -> b"abcd"
/// 14      -> b"xyz"
/// ```
///
/// and will be stored into `Properties` struct as:
/// ```ignore
/// span_ids:  [10, 10, 12, 14]
/// span_lens: [ 3,  6,  4,  3]
/// payload: b"123!@$#$%abcdxyz"
/// ```
#[derive(Debug, Clone)]
pub struct Properties {
    pub span_ids: Vec<u64>,
    pub span_lens: Vec<u64>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SpanSet {
    /// The create time of the span set. Used to calculate
    /// the waiting time of async task.
    pub create_time_ns: u64,

    /// The time corresponding to the `begin_cycles` of the first span
    pub start_time_ns: u64,

    /// Span collection
    pub spans: Vec<Span>,

    /// Property collection
    pub properties: Properties,
}
