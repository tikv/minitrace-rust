#![feature(no_more_cas)]

mod collector;
pub mod future;
mod span;
mod span_id;
pub mod time;
pub mod util;

#[cfg(feature = "fine-async")]
pub use minitrace_attribute::trace_async_fine;
pub use minitrace_attribute::{trace, trace_async};

pub use collector::*;
pub use span::*;
pub use span_id::SpanID;

#[derive(Debug, Copy, Clone)]
pub struct Span {
    pub id: u32,
    pub link: Link,
    pub elapsed_start: u32,
    pub elapsed_end: u32,
    pub tag: u32,
}

#[derive(Debug, Copy, Clone)]
pub enum Link {
    Root,
    Parent {
        id: u32,
    },
    #[cfg(feature = "fine-async")]
    Continue {
        id: u32,
    },
}
