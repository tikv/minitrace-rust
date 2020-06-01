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
pub use time::*;
pub use trace::*;
pub use trace_crossthread::*;
pub use trace_local::*;

#[cfg(test)]
mod tests;

pub use minitrace_attribute::{trace, trace_async};

#[derive(Debug, Copy, Clone)]
pub struct Span {
    pub id: u64,
    pub link: Link,
    // TODO: add cargo feature to allow altering to ns
    pub begin_cycles: u64,
    pub end_cycles: u64,
    pub event: u32,
}

#[derive(Debug, Copy, Clone)]
pub enum Link {
    Root,
    Parent { id: u64 },
    Continue { id: u64 },
}

#[derive(Debug, Clone)]
pub struct SpanSet {
    /// The create time of the span set. Used to calculate
    /// the waiting time of async task.
    pub create_time_ns: u64,

    /// The time corresponding to the `begin_cycles` of the first span
    pub start_time_ns: u64,

    /// For conversion of cycles -> ns
    pub cycles_per_sec: u64,

    pub spans: Vec<Span>,
}
