#![feature(no_more_cas)]
#![feature(negative_impls)]

mod collector;
pub mod future;
pub mod prelude;
mod span;
mod span_id;
mod time;
pub mod util;

#[cfg(feature = "fine-async")]
pub use minitrace_attribute::trace_async_fine;
pub use minitrace_attribute::{trace, trace_async};

pub use collector::*;
pub use span::*;
pub(crate) use span_id::SpanID;

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
    Root {
        start_time_ms: u64,
    },
    Parent {
        id: u32,
    },
    #[cfg(feature = "fine-async")]
    Continue {
        id: u32,
    },
}

#[macro_export]
macro_rules! block {
    ($tag:expr, $blk:block) => {{
        let span = minitrace::new_span($tag);
        let _enter = span.enter();
        $blk
    }};
}
