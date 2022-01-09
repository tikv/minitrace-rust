// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! A high-performance, ergonomic timeline tracing library for Rust.
//!
//! ## Span
//!
//!   A [`SpanRecord`] represents an individual unit of work done. It contains:
//!   - An operation name
//!   - A start timestamp and duration
//!   - A set of key-value properties
//!   - A reference to a parent `Span`
//!
//!   To record such a span record, we create a [`Span`] and drop it to stop clocking.
//!
//!   A new [`Span`] can be started via [`Span::root`], [`Span::enter_with_parent`]. The span started by the latter method will be the child span of parent.
//!
//!   [`Span`] is thread-safe and can be sent across threads.
//!
//!   ```rust
//!   use minitrace::prelude::*;
//!
//!   let (root, collector) = Span::root("root");
//!
//!   {
//!       let _child_span = Span::enter_with_parent("a child span", &root);
//!       // some work
//!   }
//!
//!   drop(root);
//!   let records: Vec<SpanRecord> = collector.collect();
//!   ```
//!
//!
//! ## Collector
//!
//!   A [`Collector`] will be provided when statring a root [`Span`]. Use it to collect all spans related to a request.
//!
//!   ```rust
//!   use minitrace::prelude::*;
//!
//!   let (root, collector) = Span::root("root");
//!   drop(root);
//!
//!   let records: Vec<SpanRecord> = collector.collect();
//!   ```
//!
//!
//! ## Local Span
//!
//!   A [`Span`] can be optimized into [`LocalSpan`], if the span is not supposed to sent to other thread, to greatly reduces the overhead.
//!
//!   Before starting a [`LocalSpan`], a scope where the parent span can be inferred from thread-local should be set using [`Span::set_local_parent`]. And then [`LocalSpan::enter_with_local_parent`] will start a [`LocalSpan`] and set it as the new local parent.
//!
//!   If the local parent is not set, [`LocalSpan`] will do nothing.
//!
//!   ```rust
//!   use minitrace::prelude::*;
//!
//!   let (root, collector) = Span::root("root");
//!
//!   {
//!       let _guard = root.set_local_parent();
//!
//!       // The parent of this span is `root`.
//!       let _span1 = LocalSpan::enter_with_local_parent("a child span");
//!
//!       {
//!           foo();
//!       }
//!   }
//!
//!   fn foo() {
//!       // The parent of this span is `span1`.
//!       let _span2 = LocalSpan::enter_with_local_parent("a child span of child span");
//!   }
//!   ```
//!
//!
//! ## Property
//!
//!   Property is an arbitrary custom kev-value pair associated to a span.
//!
//!   ```rust
//!   use minitrace::prelude::*;
//!
//!   let (mut root, collector) = Span::root("root");
//!   root.with_property(|| ("key", "value".to_owned()));
//!
//!   let _guard = root.set_local_parent();
//!
//!   let _span1 = LocalSpan::enter_with_local_parent("a child span")
//!       .with_property(|| ("key", "value".to_owned()));
//!   ```
//!
//!
//! ## Futures
//!
//!   minitrace provides [`FutureExt`] which extends [`Future`] with two methods:
//!
//!   - [`in_span`]: Bind a [`Span`] to the [`Future`] that keeps clocking until the future drops. Besides, it will set the span as the local parent at every poll so that `LocalSpan` becomes available inside the future.
//!   - [`enter_on_poll`]: Start a [`LocalSpan`] at every [`Future::poll`]. This will create multiple _short_ spans if the future get polled multiple times.
//!
//!   The outmost future must use [`in_span`] instead of [`enter_on_poll`]. Otherwise, [`enter_on_poll`] won't find a local parent at poll and thus will record nothing.
//!
//!   ```rust
//!   use minitrace::prelude::*;
//!
//!   let collector = {
//!       let (root, collector) = Span::root("root");
//!
//!       // To trace a task
//!       let task = async {
//!           async {
//!               // some work
//!           }.enter_on_poll("future is polled").await;
//!       }
//!       .in_span(Span::enter_with_parent("task", &root));
//!
//!       # let runtime = tokio::runtime::Runtime::new().unwrap();
//!       runtime.spawn(task);
//!   };
//!   ```
//!
//!
//! ## Macro
//!
//!   A attribute-macro [\#\[trace\]] is provided to help get rid of boilerplate.
//!
//!   For example:
//!
//!   ```rust
//!   use minitrace::prelude::*;
//!
//!   #[trace("foo")]
//!   fn foo() {
//!       // some work
//!   }
//!
//!   #[trace("bar")]
//!   async fn bar() {
//!       // some work
//!   }
//!
//!   #[trace("qux", enter_on_poll = true)]
//!   async fn qux() {
//!       // some work
//!   }
//!   ```
//!
//!   will be translated into
//!
//!   ```rust
//!   # use minitrace::prelude::*;
//!   fn foo() {
//!       let _span1 = LocalSpan::enter_with_local_parent("foo");
//!       // some work
//!   }
//!
//!   async fn bar() {
//!       async {
//!           // some work
//!       }
//!       .in_span(Span::enter_with_local_parent("bar"))
//!       .await
//!   }
//!
//!   async fn qux() {
//!       async {
//!           // some work
//!       }
//!       .enter_on_poll("qux")
//!       .await
//!   }
//!   ```
//!
//!   Note that [\#\[trace\]] always require an local parent in the context. For synchronous functions, make sure that the caller is within the scope of [`Span::set_local_parent`]; and for asynchronous fuctions, make sure that the caller is within a future instrumented by [`in_span`].
//!
//!
//! ## Local Collector (Advanced)
//!
//!   [`LocalCollector`] allows manully collect [`LocalSpan`] without a local parent, and the collected [`LocalSpan`] can be
//!   linked to a parent later.
//!
//!   At most time, [`Span`] and [`LocalSpan`] are sufficient. Use [`LocalCollector`] when the span may start before the parent
//!   span. Sometimes it is useful to trace the preceding task that is blocking the current request.
//!
//!   ```rust
//!   use minitrace::prelude::*;
//!   use minitrace::local::LocalCollector;
//!   use std::sync::Arc;
//!
//!   // Collect local spans in advance without parent
//!   let collector = LocalCollector::start();
//!   let _span1 = LocalSpan::enter_with_local_parent("a child span");
//!   drop(_span1);
//!   let local_spans = Arc::new(collector.collect());
//!
//!   // Link the local spans to a parent
//!   let (root, collector) = Span::root("root");
//!   root.push_child_spans(local_spans);
//!   drop(root);
//!
//!   let records: Vec<SpanRecord> = collector.collect();
//!   ```
//!
//! [`Span`]: crate::prelude::Span
//! [`LocalSpan`]: crate::prelude::LocalSpan
//! [`Collector`]: crate::prelude::Collector
//! [`SpanRecord`]: crate::prelude::SpanRecord
//! [`FutureExt`]: crate::prelude::FutureExt
//! [\#\[trace\]]: crate::prelude::trace
//! [`LocalCollector`]: crate::local::LocalCollector
//! [`Future`]: std::future::Future
//! [`Future::poll`]: std::future::Future::poll
//! [`Span::root`]: crate::prelude::Span::root
//! [`Span::enter_with_parent`]: crate::prelude::Span::enter_with_parent
//! [`Span::set_local_parent`]: crate::prelude::Span::set_local_parent
//! [`LocalSpan::enter_with_local_parent`]: crate::prelude::LocalSpan::enter_with_local_parent
//! [`in_span`]: crate::prelude::FutureExt::in_span
//! [`enter_on_poll`]: crate::prelude::FutureExt::enter_on_poll

pub mod collector;
pub mod future;
pub mod local;
pub mod span;

pub mod prelude {
    pub use crate::collector::{CollectArgs, Collector, SpanRecord};
    pub use crate::future::FutureExt as _;
    pub use crate::local::LocalSpan;
    pub use crate::span::Span;
    pub use minitrace_macro::trace;
}
