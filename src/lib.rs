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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TraceDetails {
    /// The start time of the whole tracing process that is the time
    /// when calling `trace_enable`
    pub start_time_ns: u64,

    /// The elapsed of the whole tracing process this is the time
    /// when calling `collect`
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SpanSet {
    /// The create time of the span set. Used to calculate
    /// the waiting time of async task.
    pub create_time_ns: u64,

    /// The time corresponding to the `begin_cycles` of the first span
    pub start_time_ns: u64,

    /// Span collection
    pub spans: Vec<Span>,
}
