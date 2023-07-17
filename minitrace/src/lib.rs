// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! A high-performance, ergonomic, library-level timeline tracing library for Rust.
//!
//! Most tracing libraries are designed for instrumenting executables. However, in many cases,
//! libraries also need to be instrumented for tracing purposes. `minitrace` is designed
//! to be used in libraries. It is lightweight and has zero overhead when it is not enabled in
//! the executable.
//!
//! # Quick Start
//!
//! ### In libraries
//!
//! Libraries should link to `minitrace` without enabling any extra features.
//!
//! ```toml
//! [dependencies]
//! minitrace = "0.4"
//! ```
//!
//! Libraries can attach its spans to the caller's span (if available) via the API boundary.
//!
//! ```
//! use minitrace::prelude::*;
//! # struct QueryResult;
//! # struct Error;
//!
//! struct Connection {
//!     // ...
//! }
//!
//! impl Connection {
//!     #[trace]
//!     pub fn query(sql: &str) -> Result<QueryResult, Error> {
//!         // ...
//!         # Ok(QueryResult)
//!     }
//! }
//! ```
//!
//! Also, libraries can create a new trace individually to record its work.
//!
//! ```
//! use minitrace::prelude::*;
//! # struct HttpRequest;
//! # struct Error;
//!
//! pub fn send_request(req: HttpRequest) -> Result<(), Error> {
//!     let root = Span::root(
//!         "send_request",
//!         SpanContext::new(TraceId(rand::random()), SpanId::default()),
//!     );
//!     let _guard = root.set_local_parent();
//!
//!     // ...
//!     # Ok(())
//! }
//! ```
//!
//! ### In executables
//!
//! Executables should link to `minitrace` with the `report` feature enabled. Alternatively, you
//! can also statically disable minitrace by not enabling the `report` feature.
//!
//! ```toml
//! [dependencies]
//! minitrace = { version = "0.4", features = ["report"] }
//! ```
//!
//! Executables should choose a reporter implementation and initialize it early in the runtime
//! of the program. Any span records generated before the implementation is initialized will be ignored.
//!
//! Before exiting, the reporter should be flushed to make sure all the span records are reported.
//! ```
//! use minitrace::collector::Config;
//! use minitrace::collector::ConsoleReporter;
//!
//! fn main() {
//!     minitrace::set_reporter(ConsoleReporter, Config::default());
//!
//!     // ...
//!
//!     minitrace::flush();
//! }
//! ```
//!
//!
//! # Concepts
//!
//! The basic use of `minitrace` is through three types: [`Span`], [`LocalSpan`], and [`Event`] where
//! each represents a different type of tracing record. Additionally, the macro [`trace`] is available to automatically
//! manages these three types for you. Finally, if you're using `Future`, the necessary utilities are provided by
//! [`FutureExt`] to instrument it.
//!
//!
//! ## Span
//!
//! A [`Span`] represents an individual unit of work. It contains:
//! - A name
//! - A start timestamp and duration
//! - A set of key-value properties
//! - A reference to a parent `Span`
//!
//! We create a [`Span`] to start clocking and drop it to record the duration.
//!
//! A new `Span` can be started through [`Span::root()`], you need to provide the trace id and the parent span id from
//! a remote source. If there's no remote parent span, the parent span id is typically set to its default value of zero.
//!
//! [`Span::enter_with_parent()`] will start a child span to a given parent span.
//!
//! `Span` is thread-safe and can be sent across threads.
//! ```
//! use minitrace::collector::Config;
//! use minitrace::collector::ConsoleReporter;
//! use minitrace::prelude::*;
//!
//! minitrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
//!     {
//!         let _child_span = Span::enter_with_parent("a child span", &root);
//!
//!         // Perform some work
//!     }
//! }
//!
//! minitrace::flush();
//! ```
//!
//!
//! ## Local Span
//!
//! A `Span` can be efficiently transformed into a [`LocalSpan`], provided that it is not intended for sending
//! to other threads. This transformation significantly reduces overhead.
//!
//! Before starting a `LocalSpan`, a scope of parent span should be set using [`Span::set_local_parent()`].
//! Use [`LocalSpan::enter_with_local_parent()`] to start a `LocalSpan`, and then, it will become the new local parent.
//!
//! If no local parent is set, the `enter_with_local_parent()` will do nothing.
//! ```
//! use minitrace::collector::Config;
//! use minitrace::collector::ConsoleReporter;
//! use minitrace::prelude::*;
//!
//! minitrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
//!     {
//!         let _guard = root.set_local_parent();
//!
//!         // The parent of this span is `root`.
//!         let _span1 = LocalSpan::enter_with_local_parent("a child span");
//!
//!         foo();
//!     }
//! }
//!
//! fn foo() {
//!     // The parent of this span is `span1`.
//!     let _span2 = LocalSpan::enter_with_local_parent("a child span of child span");
//! }
//!
//! minitrace::flush();
//! ```
//!
//! ## Event
//!
//! [`Event`] represent single points in time where something occurred during the execution of a program.
//! An `Event` can be seen as a log record attached to a span.
//! ```
//! use minitrace::collector::Config;
//! use minitrace::collector::ConsoleReporter;
//! use minitrace::prelude::*;
//!
//! minitrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
//!
//!     Event::add_to_parent("event in root", &root, || []);
//!
//!     {
//!         let _guard = root.set_local_parent();
//!         let mut span1 = LocalSpan::enter_with_local_parent("a child span");
//!
//!         Event::add_to_local_parent("event in span1", || [("key", "value".to_owned())]);
//!     }
//! }
//!
//! minitrace::flush();
//! ```
//!
//!
//! ## Macro
//!
//! The attribute-macro [`trace`] can help get rid of boilerplate. However, the function annotated
//! by the `trace` always requires a local parent in the context, otherwise, no span will be recorded.
//!
//! ```
//! use futures::executor::block_on;
//! use minitrace::collector::Config;
//! use minitrace::collector::ConsoleReporter;
//! use minitrace::prelude::*;
//!
//! #[trace]
//! fn do_something(i: u64) {
//!     std::thread::sleep(std::time::Duration::from_millis(i));
//! }
//!
//! #[trace]
//! async fn do_something_async(i: u64) {
//!     futures_timer::Delay::new(std::time::Duration::from_millis(i)).await;
//! }
//!
//! minitrace::set_reporter(ConsoleReporter, Config::default());
//!
//! let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
//! {
//!     let _g = root.set_local_parent();
//!
//!     do_something(100);
//!     block_on(
//!         async {
//!             do_something_async(100).await;
//!         }
//!         .in_span(Span::enter_with_local_parent("aync_job")),
//!     );
//! }
//!
//! minitrace::flush();
//! ```
//!
//!
//! ## Reporter
//!
//! [`Reporter`] is responsible for reporting the span records to remote agent, such as Jaeger.
//!
//! Executables should choose a reporter implementation and initialize it early in the runtime
//! of the program. Any span records generated before the implementation is initialized will be ignored.
//!
//! For an easy start, `minitrace` offers an [`ConsoleReporter`] who prints span records to stderr.
//! For more advanced use, crates like `minitrace-jaeger`, `minitrace-datadog`, and
//! `minitrace-opentelemetry` are available.
//!
//! By default, the reporter is triggered every 500 milliseconds. The reporter can also be triggered
//! manually by calling [`flush()`]. See [`Config`] for customizing the reporting behavior.
//! ```
//! use std::time::Duration;
//!
//! use minitrace::collector::Config;
//! use minitrace::collector::ConsoleReporter;
//!
//! minitrace::set_reporter(
//!     ConsoleReporter,
//!     Config::default().batch_report_interval(Duration::from_secs(1)),
//! );
//!
//! minitrace::flush();
//! ```
//!
//!
//! # Performance
//!
//! `minitrace` is designed to be fast and lightweight. Four scenarios are considered:
//!
//! - **No Tracing**: `minitrace` is not linked to to the executable. `minitrace`
//! in this case will be completely removed from the executable and libaries, and there is absolutely
//! zero overhead. So feel free to use `minitrace` in your libraries.
//!
//! - **Sample Tracing**: `minitrace` is enabled in the executable, but only a small
//! portion of the traces are enabled via [`Span::root()`], while the other portion start with a
//! placeholder [`Span::noop()`]. The overhead in this case is very small - merely an integer load,
//! comparison and jump.
//!
//! - **Full Tracing with Tail Sampling**: `minitrace` is enabled in the executable, and
//! all traces are enabled. However, only a select few abnormal tracing records (e.g., P99) are
//! reported. Normal traces can be dismissed by using [`Span::cancel()`] to avoid reporting. This could
//! be useful when you are interested in examining program's tail latency.
//!
//! - **Full Tracing**: `minitrace` is enabled in the executable, and all traces are
//! enabled. All tracing records are reported. `minitrace` performs 10x to 100x faster than other
//! tracing libraries in this case.
//!
//!
//! [`Span`]: crate::Span
//! [`LocalSpan`]: crate::local::LocalSpan
//! [`SpanRecord`]: crate::collector::SpanRecord
//! [`FutureExt`]: crate::future::FutureExt
//! [`trace`]: crate::trace
//! [`LocalCollector`]: crate::local::LocalCollector
//! [`Span::root()`]: crate::Span::root
//! [`Span::noop()`]: crate::Span::noop
//! [`Span::cancel()`]: crate::Span::cancel
//! [`Span::enter_with_parent()`]: crate::Span::enter_with_parent
//! [`Span::set_local_parent()`]: crate::Span::set_local_parent
//! [`LocalSpan::enter_with_local_parent()`]: crate::local::LocalSpan::enter_with_local_parent
//! [`Event`]: crate::Event
//! [`Reporter`]: crate::collector::Reporter
//! [`ConsoleReporter`]: crate::collector::ConsoleReporter
//! [`Config`]: crate::collector::Config

// Suppress a false-positive lint from clippy
// TODO: remove me once https://github.com/rust-lang/rust-clippy/issues/11076 is released
#![allow(unknown_lints)]
#![allow(clippy::arc_with_non_send_sync)]
#![allow(clippy::needless_doctest_main)]
#![cfg_attr(not(feature = "report"), allow(dead_code))]
#![cfg_attr(not(feature = "report"), allow(unused_mut))]
#![cfg_attr(not(feature = "report"), allow(unused_imports))]
#![cfg_attr(not(feature = "report"), allow(unused_variables))]

pub mod collector;
mod event;
pub mod future;
pub mod local;
mod span;
#[doc(hidden)]
pub mod util;

/// An attribute macro designed to eliminate boilerplate code.
///
/// By default, the span name is the function name. This can be customized by passing a string
/// literal as an argument.
///
/// The `#[trace]` attribute requires a local parent context to function correctly. Ensure that
/// the function annotated with `#[trace]` is called within the scope of [`Span::set_local_parent()`].
///
/// # Examples
///
/// ```
/// use minitrace::prelude::*;
///
/// #[trace]
/// fn foo() {
///     // Perform some work
/// }
///
/// #[trace]
/// async fn bar() {
///     // Perform some work
/// }
///
/// #[trace(name = "qux", enter_on_poll = true)]
/// async fn qux() {
///     // Perform some work
/// }
/// ```
///
/// The code snippets above are equivalent to:
///
/// ```
/// # use minitrace::prelude::*;
/// # use minitrace::local::LocalSpan;
/// fn foo() {
///     let __guard = LocalSpan::enter_with_local_parent("foo");
///     // Perform some work
/// }
///
/// fn bar() -> impl core::future::Future<Output = ()> {
///     async {
///         // Perform some work
///     }
///     .in_span(Span::enter_with_local_parent("bar"))
/// }
///
/// fn qux() -> impl core::future::Future<Output = ()> {
///     async {
///         // Perform some work
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
    //! A "prelude" for crates using `minitrace`.
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
