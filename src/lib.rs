// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

pub mod future;

mod collector;
mod thread;
mod trace;
mod utils;

pub use crate::collector::{collect_all, Properties, TraceResult};
pub use crate::thread::{new_async_span, AsyncGuard};
pub use crate::trace::{
    new_property, new_property_with, new_span, start_trace, RelationId, Span, SpanGuard, SpanId,
    State,
};

pub use minitrace_macro::{trace, trace_async};

// #[cfg(test)]
// mod tests;
