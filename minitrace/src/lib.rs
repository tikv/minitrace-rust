// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! `minitrace` is a high-performance, ergonomic, library-level timeline tracing library for Rust.
//!
//! Unlike most tracing libraries which are primarily designed for instrumenting executables,
//! `minitrace` also accommodates the need for library instrumentation. It stands out due to
//! its extreme lightweight and fast performance compared to other tracing libraries. Moreover,
//! it has zero overhead when not enabled in the executable, making it a worry-free choice for
//! libraries concerned about unnecessary performance loss.
//!
//! # Getting Started
//!
//! ## Libraries
//!
//! Libraries should include `minitrace` as a dependency without enabling any extra features.
//!
//! ```toml
//! [dependencies]
//! minitrace = "0.5"
//! ```
//!
//! Libraries can attach their spans to the caller's span (if available) via the API boundary.
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
//! Libraries can also create a new trace individually to record their work.
//!
//! ```
//! use minitrace::prelude::*;
//! # struct HttpRequest;
//! # struct Error;
//!
//! pub fn send_request(req: HttpRequest) -> Result<(), Error> {
//!     let root = Span::root("send_request", SpanContext::random());
//!     let _guard = root.set_local_parent();
//!
//!     // ...
//!     # Ok(())
//! }
//! ```
//!
//! ## Executables
//!
//! Executables should include `minitrace` as a dependency with the `enable` feature
//! set. To disable `minitrace` statically, simply don't set the `enable` feature.
//!
//! ```toml
//! [dependencies]
//! minitrace = { version = "0.5", features = ["enable"] }
//! ```
//!
//! Executables should initialize a reporter implementation early in the program's runtime.
//! Span records generated before the implementation is initialized will be ignored. Before
//! terminating, the reporter should be flushed to ensure all span records are reported.
//!
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
//! # Key Concepts
//!
//! `minitrace` operates through three types: [`Span`], [`LocalSpan`], and [`Event`], each
//! representing a different type of tracing record. The macro [`trace`] is available to
//! manage these types automatically. For [`Future`] instrumentation, necessary utilities
//! are provided by [`FutureExt`].
//!
//! ## Span
//!
//! A [`Span`] represents an individual unit of work. It contains:
//! - A name
//! - A start timestamp and duration
//! - A set of key-value properties
//! - A reference to a parent `Span`
//!
//! A new `Span` can be started through [`Span::root()`], requiring the trace id and the
//! parent span id from a remote source. If there's no remote parent span, the parent span
//! id is typically set to its default value of zero.
//!
//! [`Span::enter_with_parent()`] starts a child span given a parent span.
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
//!     let root_span = Span::root("root", SpanContext::random());
//!
//!     {
//!         let child_span = Span::enter_with_parent("a child span", &root_span);
//!
//!         // ...
//!
//!         // child_span ends here.
//!     }
//!
//!     // root_span ends here.
//! }
//!
//! minitrace::flush();
//! ```
//!
//! ## Local Span
//!
//! A `Span` can be efficiently replaced with a [`LocalSpan`], reducing overhead
//! significantly, provided it is not intended for sending to other threads.
//!
//! Before starting a `LocalSpan`, a scope of parent span should be set using
//! [`Span::set_local_parent()`]. Use [`LocalSpan::enter_with_local_parent()`] to start
//! a `LocalSpan`, which then becomes the new local parent.
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
//!     let root = Span::root("root", SpanContext::random());
//!
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
//! [`Event`] represents a single point in time where something occurred during the execution of a program.
//!
//! An `Event` can be seen as a log record attached to a span.
//! ```
//! use minitrace::collector::Config;
//! use minitrace::collector::ConsoleReporter;
//! use minitrace::prelude::*;
//!
//! minitrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root = Span::root("root", SpanContext::random());
//!
//!     Event::add_to_parent("event in root", &root, || []);
//!
//!     {
//!         let _guard = root.set_local_parent();
//!         let _span1 = LocalSpan::enter_with_local_parent("a child span");
//!
//!         Event::add_to_local_parent("event in span1", || [("key".into(), "value".into())]);
//!     }
//! }
//!
//! minitrace::flush();
//! ```
//!
//! ## Macro
//!
//! The attribute-macro [`trace`] helps to reduce boilerplate. However, the function annotated
//! by the `trace` always requires a local parent in the context, otherwise, no span will be
//! recorded.
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
//! {
//!     let root = Span::root("root", SpanContext::random());
//!     let _guard = root.set_local_parent();
//!
//!     do_something(100);
//!
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
//! ## Reporter
//!
//! [`Reporter`] is responsible for reporting the span records to a remote agent,
//! such as Jaeger.
//!
//! Executables should initialize a reporter implementation early in the program's
//! runtime. Span records generated before the implementation is initialized will be ignored.
//!
//! For an easy start, `minitrace` offers a [`ConsoleReporter`] that prints span
//! records to stderr. For more advanced use, crates like `minitrace-jaeger`, `minitrace-datadog`,
//! and `minitrace-opentelemetry` are available.
//!
//! By default, the reporter is triggered every 500 milliseconds. The reporter can also be
//! triggered manually by calling [`flush()`]. See [`Config`] for customizing the reporting behavior.
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
//! # Performance
//!
//! `minitrace` is designed to be fast and lightweight, considering four scenarios:
//!
//! - **No Tracing**: `minitrace` is not included as dependency in the executable, while the
//! libraries has been intrumented. In this case, it will be completely removed from libraries,
//! causing zero overhead.
//!
//! - **Sample Tracing**: `minitrace` is enabled in the executable, but only a small portion
//! of the traces are enabled via [`Span::root()`], while the other portion start with placeholders
//! by [`Span::noop()`]. The overhead in this case is very small - merely an integer
//! load, comparison, and jump.
//!
//! - **Full Tracing with Tail Sampling**: `minitrace` is enabled in the executable, and all
//! traces are enabled. However, only a select few abnormal tracing records (e.g., P99) are
//! reported. Normal traces can be dismissed by using [`Span::cancel()`] to avoid reporting.
//! This could be useful when you are interested in examining program's tail latency.
//!
//! - **Full Tracing**: `minitrace` is enabled in the executable, and all traces are enabled.
//! All tracing records are reported. `minitrace` performs 10x to 100x faster than other tracing
//! libraries in this case.
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
//! [`Future`]: std::future::Future

// Suppress a false-positive lint from clippy
// TODO: remove me once https://github.com/rust-lang/rust-clippy/issues/11076 is released
#![allow(unknown_lints)]
#![allow(clippy::arc_with_non_send_sync)]
#![allow(clippy::needless_doctest_main)]
#![cfg_attr(not(feature = "enable"), allow(dead_code))]
#![cfg_attr(not(feature = "enable"), allow(unused_mut))]
#![cfg_attr(not(feature = "enable"), allow(unused_imports))]
#![cfg_attr(not(feature = "enable"), allow(unused_variables))]

pub mod collector;
mod event;
pub mod future;
pub mod local;
mod span;
#[doc(hidden)]
pub mod util;

pub use minitrace_macro::trace;

pub use crate::collector::global_collector::flush;
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
