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
//!   A new [`Span`] can be started via [`Span::root(event)`](crate::prelude::Span::root), [`Span::enter_with_parent(event, parent)`](crate::prelude::Span::enter_with_parent). The span started by the latter method will be the child span of parent.
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
//!       // some works
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
//! ## Local Span & Local Parent Guard
//!
//!   A [`Span`] can be optimized into [`LocalSpan`], if the span is not supposed to sent to other thread, to greatly reduces the overhead.
//!
//!   Before starting a [`LocalSpan`], a scope where the parent span can be inferred from thread-local should be set using [`Span::set_local_parent()`](crate::prelude::Span::set_local_parent). And then a [`LocalSpan`] can start by [`LocalSpan::enter_with_local_parent()`](crate::prelude::LocalSpan::enter_with_local_parent).
//!
//!   If the local parent is not set, the [`LocalSpan`] will panic on debug profile or do nothing on release profile.
//!
//!   ```rust
//!   use minitrace::prelude::*;
//!
//!   let (root, collector) = Span::root("root");
//!
//!   {
//!       let _local_parent_guard = root.set_local_parent();
//!
//!       // The parent of this span is `root`.
//!       let _span_guard = LocalSpan::enter_with_local_parent("a child span");
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
//!   let _local_parent_guard = root.set_local_parent();
//!
//!   let _span_guard = LocalSpan::enter_with_local_parent("a child span")
//!       .with_property(|| ("key", "value".to_owned()));
//!   ```
//!
//!
//! ## Futures
//!
//!   minitrace provides [`FutureExt`] which extends [`Future`] with two methods:
//!
//!   - [`in_span`](crate::prelude::FutureExt::in_span): Bind a [`Span`] that stop clocking when the [`Future`] drops. Besides, it'll call `Span::set_local_parent` at every poll.
//!   - [`enter_on_poll`](crate::prelude::FutureExt::enter_on_poll): Start on local span at every poll.
//!
//!   The [`in_span`](crate::prelude::FutureExt::in_span) adaptor is commonly used on the outmost [`Future`] which is about to submit to a runtime.
//!
//!   ```rust
//!   use minitrace::prelude::*;
//!
//!   let collector = {
//!       let (root, collector) = Span::root("root");
//!
//!       // To trace another task
//!       let task = async {
//!           async {
//!               // some works
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
//!   The two attribute macros [`trace`] and [`trace_async`] for `fn` is provided to help get rid of boilerplate.
//!
//!   - [`trace`]
//!
//!     For example, the code list below has been annotated with a event name:
//!
//!     ```rust
//!     use minitrace::prelude::*;
//!
//!     #[trace("wow")]
//!     fn amazing_func() {
//!         // some works
//!     }
//!     ```
//!
//!     which will be translated into
//!
//!     ```rust
//!     use minitrace::prelude::*;
//!
//!     fn amazing_func() {
//!         let _span_guard = LocalSpan::enter_with_local_parent("wow");
//!         // some works
//!     }
//!     ```
//!
//!   - [`trace_async`]
//!
//!     Similarly, `async fn` uses [`trace_async`]:
//!
//!     ```rust
//!     use minitrace::prelude::*;
//!
//!     #[trace_async("wow")]
//!     async fn amazing_func() {
//!         // some works
//!     }
//!     ```
//!
//!     which will be translated into
//!
//!     ```rust
//!     use minitrace::prelude::*;
//!
//!     async fn amazing_func() {
//!         async {
//!             // some works
//!         }
//!         .enter_on_poll("wow")
//!         .await
//!     }
//!     ```
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
//!   // Collect local spans in advance with no parent
//!   let collector = LocalCollector::start().unwrap();
//!   let _span_guard = LocalSpan::enter_with_local_parent("a child span");
//!   drop(_span_guard);
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
//! [`trace`]: crate::prelude::trace
//! [`trace_async`]: crate::prelude::trace_async
//! [`LocalCollector`]: crate::local::LocalCollector
//! [`Future`]: std::future::Future

#![allow(clippy::return_self_not_must_use)]

pub mod collector;
pub mod future;
pub mod local;
pub mod span;

pub mod prelude {
    pub use crate::collector::{CollectArgs, Collector, SpanRecord};
    pub use crate::future::FutureExt;
    pub use crate::local::LocalSpan;
    pub use crate::span::Span;
    pub use minitrace_macro::{trace, trace_async};
}

#[cfg(test)]
mod tests {
    use super::prelude::*;
    use crate::collector::CollectArgs;
    use crate::local::local_collector::LocalCollector;
    use std::sync::Arc;

    fn four_spans() {
        {
            // wide
            for _ in 0..2 {
                let _g = LocalSpan::enter_with_local_parent("iter span")
                    .with_property(|| ("tmp_property", "tmp_value".into()));
            }
        }

        {
            #[trace("rec span")]
            fn rec(mut i: u32) {
                i -= 1;

                if i > 0 {
                    rec(i);
                }
            }

            // deep
            rec(2);
        }
    }

    #[test]
    fn single_thread_single_span() {
        let spans = {
            let (root_span, collector) = Span::root("root");
            let _g = root_span.set_local_parent();

            four_spans();

            collector
        }
        .collect_with_args(CollectArgs::default().sync(true));

        assert_eq!(spans.len(), 5);
    }

    #[test]
    fn single_thread_multiple_spans() {
        let (spans1, spans2, spans3) = {
            let (c1, c2, c3) = {
                let (root_span1, collector1) = Span::root("root1");
                let (root_span2, collector2) = Span::root("root2");
                let (root_span3, collector3) = Span::root("root3");

                let local_collector = LocalCollector::start().unwrap();

                four_spans();

                let local_spans = Arc::new(local_collector.collect());

                root_span1.push_child_spans(local_spans.clone());
                root_span2.push_child_spans(local_spans.clone());
                root_span3.push_child_spans(local_spans);

                (collector1, collector2, collector3)
            };

            (
                c1.collect_with_args(CollectArgs::default().sync(true)),
                c2.collect_with_args(CollectArgs::default().sync(true)),
                c3.collect_with_args(CollectArgs::default().sync(true)),
            )
        };

        assert_eq!(spans1.len(), 5);
        assert_eq!(spans2.len(), 5);
        assert_eq!(spans3.len(), 5);
    }

    #[test]
    fn multiple_threads_single_span() {
        let spans = {
            let (span, collector) = Span::root("root");
            let _g = span.set_local_parent();

            for _ in 0..4 {
                let child_span = Span::enter_with_local_parent("cross-thread");
                std::thread::spawn(move || {
                    let _g = child_span.set_local_parent();
                    four_spans();
                });
            }

            four_spans();

            collector
        }
        .collect_with_args(CollectArgs::default().sync(true));

        assert_eq!(spans.len(), 25);
    }

    #[test]
    fn multiple_threads_multiple_spans() {
        let (spans1, spans2) = {
            let (c1, c2) = {
                let (root_span1, collector1) = Span::root("root1");
                let (root_span2, collector2) = Span::root("root2");
                let local_collector = LocalCollector::start().unwrap();

                for _ in 0..4 {
                    let merged = Span::enter_with_parents(
                        "merged",
                        vec![&root_span1, &root_span2].into_iter(),
                    );
                    std::thread::spawn(move || {
                        let local_collector = LocalCollector::start().unwrap();

                        four_spans();

                        let local_spans = Arc::new(local_collector.collect());
                        merged.push_child_spans(local_spans);
                    });
                }

                four_spans();

                let local_spans = Arc::new(local_collector.collect());
                root_span1.push_child_spans(local_spans.clone());
                root_span2.push_child_spans(local_spans);
                (collector1, collector2)
            };

            (
                c1.collect_with_args(CollectArgs::default().sync(true)),
                c2.collect_with_args(CollectArgs::default().sync(true)),
            )
        };

        assert_eq!(spans1.len(), 25);
        assert_eq!(spans2.len(), 25);
    }

    #[test]
    fn multiple_spans_without_local_spans() {
        let (spans1, spans2, spans3) = {
            let (c1, c2, c3) = {
                let (root_span1, collector1) = Span::root("root1");
                let (root_span2, collector2) = Span::root("root2");
                let (root_span3, collector3) = Span::root("root3");

                let local_collector = LocalCollector::start().unwrap();

                let local_spans = Arc::new(local_collector.collect());
                root_span1.push_child_spans(local_spans.clone());
                root_span2.push_child_spans(local_spans.clone());
                root_span3.push_child_spans(local_spans);

                (collector1, collector2, collector3)
            };

            (
                c1.collect_with_args(CollectArgs::default().sync(true)),
                c2.collect_with_args(CollectArgs::default().sync(true)),
                c3.collect_with_args(CollectArgs::default().sync(true)),
            )
        };

        assert_eq!(spans1.len(), 1);
        assert_eq!(spans2.len(), 1);
        assert_eq!(spans3.len(), 1);
    }
}
