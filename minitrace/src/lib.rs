// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! A high-performance, ergonomic timeline tracing library for Rust.
//!
//! ## Span
//!
//!   A [`SpanRecord`] represents an individual unit of work. It contains:
//!   - An operation name
//!   - A start timestamp and duration
//!   - A set of key-value properties
//!   - A reference to a parent `Span`
//!
//!   To record such a span record, we create a [`Span`] to start clocking and drop it to stop recording.
//!
//!   A new `Span` can be started via [`Span::root()`] and [`Span::enter_with_parent()`]. `Span::enter_with_parent()`
//!   will start a child span to a given parent span.
//!
//!   `Span` is thread-safe and can be sent across threads.
//!
//!   ```
//!   use minitrace::prelude::*;
//!   use futures::executor::block_on;
//!
//!   let (root, collector) = Span::root("root");
//!
//!   {
//!       let _child_span = Span::enter_with_parent("a child span", &root);
//!       // some work
//!   }
//!
//!   drop(root);
//!   let records: Vec<SpanRecord> = block_on(collector.collect());
//!
//!   println!("{records:#?}");
//!   // [
//!   //     SpanRecord {
//!   //         id: 1,
//!   //         parent_id: 0,
//!   //         begin_unix_time_ns: 1642166520139678013,
//!   //         duration_ns: 16008,
//!   //         name: "root",
//!   //         properties: [],
//!   //         events: [],
//!   //     },
//!   //     SpanRecord {
//!   //         id: 2,
//!   //         parent_id: 1,
//!   //         begin_unix_time_ns: 1642166520139692070,
//!   //         duration_ns: 634,
//!   //         name: "a child span",
//!   //         properties: [],
//!   //         events: [],
//!   //     },
//!   // ]
//!   ```
//!
//!
//! ## Local Span
//!
//!   A `Span` can be optimized into [`LocalSpan`], if the span is not supposed to be sent to other threads,
//!   which can greatly reduce the overhead.
//!
//!   Before starting a `LocalSpan`, a scope of parent span should be set using [`Span::set_local_parent()`].
//!   Use [`LocalSpan::enter_with_local_parent()`] to start a `LocalSpan`, and then, it will become the new local parent.
//!
//!   If no local parent is set, the `enter_with_local_parent()` will do nothing.
//!
//!   ```
//!   use minitrace::prelude::*;
//!   use futures::executor::block_on;
//!
//!   let (root, collector) = Span::root("root");
//!
//!   {
//!       let _guard = root.set_local_parent();
//!
//!       // The parent of this span is `root`.
//!       let _span1 = LocalSpan::enter_with_local_parent("a child span");
//!
//!       foo();
//!   }
//!
//!   fn foo() {
//!       // The parent of this span is `span1`.
//!       let _span2 = LocalSpan::enter_with_local_parent("a child span of child span");
//!   }
//!
//!   drop(root);
//!   let records: Vec<SpanRecord> = block_on(collector.collect());
//!
//!   println!("{records:#?}");
//!   // [
//!   //     SpanRecord {
//!   //         id: 1,
//!   //         parent_id: 0,
//!   //         begin_unix_time_ns: 1643101008017429580,
//!   //         duration_ns: 64132,
//!   //         name: "root",
//!   //         properties: [],
//!   //         events: [],
//!   //     },
//!   //     SpanRecord {
//!   //         id: 2,
//!   //         parent_id: 1,
//!   //         begin_unix_time_ns: 1643101008017486383,
//!   //         duration_ns: 4150,
//!   //         name: "a child span",
//!   //         properties: [],
//!   //         events: [],
//!   //     },
//!   //     SpanRecord {
//!   //         id: 3,
//!   //         parent_id: 2,
//!   //         begin_unix_time_ns: 1643101008017488703,
//!   //         duration_ns: 1318,
//!   //         name: "a child span of child span",
//!   //         properties: [],
//!   //         events: [],
//!   //     },
//!   // ]
//!   ```
//!
//!
//! ## Property
//!
//!   Property is an arbitrary custom kev-value pair associated to a span.
//!
//!   ```
//!   use minitrace::prelude::*;
//!   use futures::executor::block_on;
//!
//!   let (mut root, collector) = Span::root("root");
//!   root.add_property(|| ("key", "value".to_owned()));
//!
//!   {
//!       let _guard = root.set_local_parent();
//!
//!       let mut span1 = LocalSpan::enter_with_local_parent("a child span");
//!       span1.add_property(|| ("key", "value".to_owned()));
//!   }
//!
//!   drop(root);
//!   let records: Vec<SpanRecord> = block_on(collector.collect());
//!
//!   println!("{records:#?}");
//!   // [
//!   //     SpanRecord {
//!   //         id: 1,
//!   //         parent_id: 0,
//!   //         begin_unix_time_ns: 1642166791041022255,
//!   //         duration_ns: 121705,
//!   //         name: "root",
//!   //         properties: [
//!   //             (
//!   //                 "key",
//!   //                 "value",
//!   //             ),
//!   //         ],
//!   //         events: [],
//!   //     },
//!   //     SpanRecord {
//!   //         id: 2,
//!   //         parent_id: 1,
//!   //         begin_unix_time_ns: 1642166791041132550,
//!   //         duration_ns: 7724,
//!   //         name: "a child span",
//!   //         properties: [
//!   //             (
//!   //                 "key",
//!   //                 "value",
//!   //             ),
//!   //         ],
//!   //         events: [],
//!   //     },
//!   // ]
//!   ```
//!
//!
//! ## Event
//!
//!   [`Event`] represent single points in time where something occurred during the execution of a program.
//!   An `Event` can be seen as a log record attached to a span.
//!
//!   ```
//!   use minitrace::prelude::*;
//!   use futures::executor::block_on;
//!
//!   let (mut root, collector) = Span::root("root");
//!
//!   Event::add_to_parent("event in root", &root, || []);
//!
//!   {
//!       let _guard = root.set_local_parent();
//!       let mut span1 = LocalSpan::enter_with_local_parent("a child span");
//!
//!       Event::add_to_local_parent("event in span1", || [("key", "value".to_owned())]);
//!   }
//!
//!   drop(root);
//!   let records: Vec<SpanRecord> = block_on(collector.collect());
//!
//!   println!("{records:#?}");
//!   // [
//!   //     SpanRecord {
//!   //         id: 1,
//!   //         parent_id: 0,
//!   //         begin_unix_time_ns: 1689321940550848459,
//!   //         duration_ns: 25708,
//!   //         name: "root",
//!   //         properties: [],
//!   //         events: [
//!   //             EventRecord {
//!   //                 name: "event in root",
//!   //                 timestamp_unix_ns: 1689321940550870667,
//!   //                 properties: [],
//!   //             },
//!   //         ],
//!   //     },
//!   //     SpanRecord {
//!   //         id: 3,
//!   //         parent_id: 1,
//!   //         begin_unix_time_ns: 1689321940550874167,
//!   //         duration_ns: 0,
//!   //         name: "span1",
//!   //         properties: [],
//!   //         events: [
//!   //             EventRecord {
//!   //                 name: "event in span1",
//!   //                 timestamp_unix_ns: 1689321940550874167,
//!   //                 properties: [
//!   //                     (
//!   //                         "key",
//!   //                         "value",
//!   //                     ),
//!   //                 ],
//!   //             },
//!   //         ],
//!   //     },
//!   // ]
//!   ```
//!
//!
//! ## Macro
//!
//!   An attribute-macro [`trace`] can help get rid of boilerplate. The macro always requires a local
//!   parent in the context, otherwise, no span will be recorded.
//!
//!   ```
//!   use minitrace::prelude::*;
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
//!   let (root, collector) = Span::root("root");
//!
//!   {
//!       let _g = root.set_local_parent();
//!       do_something(100);
//!       block_on(do_something_async(100));
//!   }
//!
//!   drop(root);
//!   let records: Vec<SpanRecord> = block_on(collector.collect());
//!
//!   println!("{records:#?}");
//!   // [
//!   //     SpanRecord {
//!   //         id: 1,
//!   //         parent_id: 0,
//!   //         begin_unix_time_ns: 1642167988459480418,
//!   //         duration_ns: 200741472,
//!   //         name: "root",
//!   //         properties: [],
//!   //     },
//!   //     SpanRecord {
//!   //         id: 2,
//!   //         parent_id: 1,
//!   //         begin_unix_time_ns: 1642167988459571971,
//!   //         duration_ns: 100084126,
//!   //         name: "do_something",
//!   //         properties: [],
//!   //     },
//!   //     SpanRecord {
//!   //         id: 3,
//!   //         parent_id: 1,
//!   //         begin_unix_time_ns: 1642167988559887219,
//!   //         duration_ns: 100306947,
//!   //         name: "do_something_async",
//!   //         properties: [],
//!   //     },
//!   // ]
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

// Suppress a false-positive lint from clippy
// TODO: remove me once https://github.com/rust-lang/rust-clippy/issues/11076 is released
#![allow(unknown_lints)]
#![allow(clippy::arc_with_non_send_sync)]

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

pub use crate::event::Event;
pub use crate::span::Span;

pub mod prelude {
    //! A "prelude" for crates using the `minitrace` crate.
    #[doc(no_inline)]
    pub use crate::collector::CollectArgs;
    #[doc(no_inline)]
    pub use crate::collector::Collector;
    #[doc(no_inline)]
    pub use crate::collector::SpanRecord;
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
