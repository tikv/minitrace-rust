// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![feature(negative_impls)]

pub use crate::future::FutureExt;
pub use crate::local::local_collector::{LocalCollector, RawSpans};
pub use crate::local::scope_guard::LocalScopeGuard;
pub use crate::local::span_guard::LocalSpanGuard;
pub use crate::span::cycle;
pub use crate::span::Span;
pub use crate::trace::collector::{CollectArgs, Collector};
pub use crate::trace::scope::Scope;

pub(crate) mod future;
pub(crate) mod local;
pub(crate) mod span;
pub(crate) mod trace;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::local_collector::LocalCollector;
    use crate::trace::collector::CollectArgs;
    use minitrace_macro::trace;
    use std::sync::Arc;

    fn four_spans() {
        {
            // wide
            for _ in 0..2 {
                let _g =
                    Span::enter("iter span").with_property(|| ("tmp_property", "tmp_value".into()));
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
    fn single_thread_single_scope() {
        let spans = {
            let (root_scope, collector) = Scope::root("root");
            let _g = root_scope.enter();

            four_spans();

            collector
        }
        .collect_with_args(CollectArgs::default().sync(true));

        assert_eq!(spans.len(), 5);
    }

    #[test]
    fn single_thread_multiple_scopes() {
        let (spans1, spans2, spans3) = {
            let (c1, c2, c3) = {
                let (root_scope1, collector1) = Scope::root("root1");
                let (root_scope2, collector2) = Scope::root("root2");
                let (root_scope3, collector3) = Scope::root("root3");

                let local_collector = LocalCollector::start();

                four_spans();

                let raw_spans = Arc::new(local_collector.collect());

                root_scope1.extend_raw_spans(raw_spans.clone());
                root_scope2.extend_raw_spans(raw_spans.clone());
                root_scope3.extend_raw_spans(raw_spans);

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
    fn multiple_threads_single_scope() {
        let spans = {
            let (scope, collector) = Scope::root("root");
            let _g = scope.enter();

            for _ in 0..4 {
                let child_scope = Scope::from_local_parent("cross-thread");
                std::thread::spawn(move || {
                    let _g = child_scope.enter();
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
    fn multiple_threads_multiple_scopes() {
        let (spans1, spans2) = {
            let (c1, c2) = {
                let (root_scope1, collector1) = Scope::root("root1");
                let (root_scope2, collector2) = Scope::root("root2");
                let local_collector = LocalCollector::start();

                for _ in 0..4 {
                    let merged =
                        Scope::from_parents("merged", vec![&root_scope1, &root_scope2].into_iter());
                    std::thread::spawn(move || {
                        let local_collector = LocalCollector::start();

                        four_spans();

                        let raw_spans = Arc::new(local_collector.collect());
                        merged.extend_raw_spans(raw_spans);
                    });
                }

                four_spans();

                let raw_spans = Arc::new(local_collector.collect());
                root_scope1.extend_raw_spans(raw_spans.clone());
                root_scope2.extend_raw_spans(raw_spans);
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
    fn multiple_scopes_without_spans() {
        let (spans1, spans2, spans3) = {
            let (c1, c2, c3) = {
                let (root_scope1, collector1) = Scope::root("root1");
                let (root_scope2, collector2) = Scope::root("root2");
                let (root_scope3, collector3) = Scope::root("root3");

                let local_collector = LocalCollector::start();

                let raw_spans = Arc::new(local_collector.collect());
                root_scope1.extend_raw_spans(raw_spans.clone());
                root_scope2.extend_raw_spans(raw_spans.clone());
                root_scope3.extend_raw_spans(raw_spans);

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
