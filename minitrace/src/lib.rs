// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! A high-performance, ergonomic timeline tracing library for Rust.
//!
//! ## Span
//!
//!   A [`Span`] represents an individual unit of work. It contains:
//!   - An operation name
//!   - A start timestamp and duration
//!   - A set of key-value properties
//!   - A reference to a parent `Span`
//!
//!   We create a [`Span`] to start clocking and drop it to record it.
//!
//!   A new `Span` can be started via [`Span::root()`], where the trace id and the parent
//!   span id from remote should be provided. If there is no remote parent span,
//!   the parent span id is usually set to default, which is zero.
//!
//!   [`Span::enter_with_parent()`] will start a child span to a given parent span.
//!
//!   `Span` is thread-safe and can be sent across threads.
//!
//!   ```
//!   use minitrace::prelude::*;
//!   use minitrace::collector::TerminalReporter;
//!   use minitrace::collector::Config;
//!
//!   minitrace::set_reporter(TerminalReporter, Config::default());
//!
//!   {
//!       let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
//!       {
//!           let _child_span = Span::enter_with_parent("a child span", &root);
//!
//!           // some work
//!       }
//!   }
//!
//!   minitrace::flush();
//!   ```
//!
//!
//! ## Local Span
//!
//!   A `Span` can be optimized into a [`LocalSpan`], if the span is not supposed to be sent to other threads,
//!   which can greatly reduce the overhead.
//!
//!   Before starting a `LocalSpan`, a scope of parent span should be set using [`Span::set_local_parent()`].
//!   Use [`LocalSpan::enter_with_local_parent()`] to start a `LocalSpan`, and then, it will become the new local parent.
//!
//!   If no local parent is set, the `enter_with_local_parent()` will do nothing.
//!
//!   ```
//!   use minitrace::prelude::*;
//!   use minitrace::collector::TerminalReporter;
//!   use minitrace::collector::Config;
//!
//!   minitrace::set_reporter(TerminalReporter, Config::default());
//!
//!   {
//!       let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
//!       {
//!           let _guard = root.set_local_parent();
//!    
//!           // The parent of this span is `root`.
//!           let _span1 = LocalSpan::enter_with_local_parent("a child span");
//!    
//!           foo();
//!       }
//!   }
//!
//!   fn foo() {
//!       // The parent of this span is `span1`.
//!       let _span2 = LocalSpan::enter_with_local_parent("a child span of child span");
//!   }
//!
//!   minitrace::flush();
//!   ```
//!
//! ## Event
//!
//!   [`Event`] represent single points in time where something occurred during the execution of a program.
//!   An `Event` can be seen as a log record attached to a span.
//!
//!   ```
//!   use minitrace::prelude::*;
//!   use minitrace::collector::TerminalReporter;
//!   use minitrace::collector::Config;
//!
//!   minitrace::set_reporter(TerminalReporter, Config::default());
//!
//!   {
//!       let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
//!
//!       Event::add_to_parent("event in root", &root, || []);
//!
//!       {
//!           let _guard = root.set_local_parent();
//!           let mut span1 = LocalSpan::enter_with_local_parent("a child span");
//!    
//!           Event::add_to_local_parent("event in span1", || [("key", "value".to_owned())]);
//!       }
//!   }
//!
//!   minitrace::flush();
//!   ```
//!
//!
//! ## Macro
//!
//!   An attribute-macro [`trace`] can help get rid of boilerplate. The function annotated by the macro
//!   always requires a local parent in the context, otherwise, no span will be recorded.
//!
//!   ```
//!   use minitrace::prelude::*;
//!   use minitrace::collector::TerminalReporter;
//!   use minitrace::collector::Config;
//!   use futures::executor::block_on;
//!
//!   #[trace]
//!   fn do_something(i: u64) {
//!       std::thread::sleep(std::time::Duration::from_millis(i));
//!   }
//!
//!   #[trace]
//!   async fn do_something_async(i: u64) {
//!       futures_timer::Delay::new(std::time::Duration::from_millis(i)).await;
//!   }
//!
//!   minitrace::set_reporter(TerminalReporter, Config::default());
//!
//!   let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
//!   {
//!       let _g = root.set_local_parent();
//!   
//!       do_something(100);
//!       block_on(
//!           async {
//!               do_something_async(100).await;
//!           }
//!           .in_span(Span::enter_with_local_parent("aync_job")),
//!       );
//!   }
//!
//!   minitrace::flush();
//!   ```
//!
//!
//! ## Reporter
//!
//!   A [`Reporter`] is responsible for collecting and reporting the span records.
// As Spans and Events transpire, they are gathered by minitrace's global collector. They are then reported to a remote collector agent such as Jaeger, using the [`Reporter`].

// Executables should choose a reporter implementation and initialize it early in the runtime of the program. Any tracing records generated before the implementation is initialized will be ignored.

// If no reporter implementation is selected, the facade falls back to a “noop” implementation that ignores all log messages. The overhead zero.
//!   ```
//!   use minitrace::collector::TerminalReporter;
//!   use minitrace::collector::Config;
//!
//!   minitrace::set_reporter(TerminalReporter, Config::default());
//!
//!   minitrace::flush();
//!   ```
//!
//! [`Span`]: crate::Span
//! [`LocalSpan`]: crate::local::LocalSpan
//! [`SpanRecord`]: crate::collector::SpanRecord
//! [`FutureExt`]: crate::future::FutureExt
//! [`trace`]: crate::trace
//! [`LocalCollector`]: crate::local::LocalCollector
//! [`Span::root()`]: crate::Span::root
//! [`Span::enter_with_parent()`]: crate::Span::enter_with_parent
//! [`Span::set_local_parent()`]: crate::Span::set_local_parent
//! [`LocalSpan::enter_with_local_parent()`]: crate::local::LocalSpan::enter_with_local_parent
//! [`Event`]: crate::Event
//! [`Reporter`]: crate::collector::Reporter

// Suppress a false-positive lint from clippy
// TODO: remove me once https://github.com/rust-lang/rust-clippy/issues/11076 is released
#![allow(unknown_lints)]
#![allow(clippy::arc_with_non_send_sync)]
#![cfg_attr(not(feature = "report"), allow(dead_code))]
#![cfg_attr(not(feature = "report"), allow(unused_imports))]
#![cfg_attr(not(feature = "report"), allow(unused_variables))]

pub mod collector;
mod event;
pub mod future;
pub mod local;
mod span;
#[doc(hidden)]
pub mod util;

/// An attribute-macro to help get rid of boilerplate.
///
/// The span name is the function name by default. It can be customized by passing a string literal.
///
/// [`trace`] always require an local parent in the context. Make sure that the caller
/// is within the scope of [`Span::set_local_parent()`].
///
/// # Examples
///
/// ```
/// use minitrace::prelude::*;
///
/// #[trace]
/// fn foo() {
///     // some work
/// }
///
/// #[trace]
/// async fn bar() {
///     // some work
/// }
///
/// #[trace("qux", enter_on_poll = true)]
/// async fn qux() {
///     // some work
/// }
/// ```
///
/// The examples above will be translated into:
///
/// ```
/// # use minitrace::prelude::*;
/// # use minitrace::local::LocalSpan;
/// fn foo() {
///     let __guard = LocalSpan::enter_with_local_parent("foo");
///     // some work
/// }
///
/// fn bar() -> impl core::future::Future<Output = ()> {
///     async {
///         // some work
///     }
///     .in_span(Span::enter_with_local_parent("bar"))
/// }
///
/// fn qux() -> impl core::future::Future<Output = ()> {
///     async {
///         // some work
///     }
///     .enter_on_poll("qux")
/// }
/// ```
///
/// [`in_span()`]: crate::future::FutureExt::in_span
pub use minitrace_macro::trace;

#[cfg(feature = "report")]
pub use crate::collector::global_collector::flush;
#[cfg(feature = "report")]
pub use crate::collector::global_collector::set_reporter;
pub use crate::event::Event;
pub use crate::span::Span;

pub mod prelude {
    //! A "prelude" for crates using the `minitrace` crate.
    #[doc(no_inline)]
    pub use crate::collector::SpanContext;
    #[doc(no_inline)]
    pub use crate::collector::SpanId;
    #[doc(no_inline)]
    pub use crate::collector::SpanRecord;
    #[doc(no_inline)]
    pub use crate::collector::TraceId;
    #[doc(no_inline)]
    pub use crate::event::Event;
    #[doc(no_inline)]
    pub use crate::future::FutureExt as _;
    #[doc(no_inline)]
    pub use crate::local::LocalSpan;
    #[doc(no_inline)]
    pub use crate::span::Span;
    #[doc(no_inline)]
    pub use crate::trace;
}
