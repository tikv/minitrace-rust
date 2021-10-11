// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

pub use crate::future::FutureExt;
pub use crate::local::local_collector::{LocalCollector, LocalSpans};
pub use crate::local::local_span_guard::LocalSpanGuard;
pub use crate::local::span_guard::SpanGuard;
pub use crate::trace::collector::{CollectArgs, Collector};
pub use crate::trace::local_span::LocalSpan;
pub use crate::trace::span::Span;

pub mod span;

pub(crate) mod future;
pub(crate) mod local;
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
                let _g = LocalSpan::enter("iter span".to_owned())
                    .with_property(|| ("tmp_property".into(), "tmp_value".into()));
            }
        }

        {
            #[trace("rec span".to_owned())]
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
            let (root_span, collector) = Span::root("root".to_owned());
            let _g = root_span.enter();

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
                let (root_span1, collector1) = Span::root("root1".to_owned());
                let (root_span2, collector2) = Span::root("root2".to_owned());
                let (root_span3, collector3) = Span::root("root3".to_owned());

                let local_collector = LocalCollector::start();

                four_spans();

                let local_spans = Arc::new(local_collector.collect());

                root_span1.mount_local_spans(local_spans.clone());
                root_span2.mount_local_spans(local_spans.clone());
                root_span3.mount_local_spans(local_spans);

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
            let (span, collector) = Span::root("root".to_owned());
            let _g = span.enter();

            for _ in 0..4 {
                let child_span = Span::from_local_parent("cross-thread".to_owned());
                std::thread::spawn(move || {
                    let _g = child_span.enter();
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
                let (root_span1, collector1) = Span::root("root1".to_owned());
                let (root_span2, collector2) = Span::root("root2".to_owned());
                let local_collector = LocalCollector::start();

                for _ in 0..4 {
                    let merged =
                        Span::from_parents("merged".to_owned(), vec![&root_span1, &root_span2].into_iter());
                    std::thread::spawn(move || {
                        let local_collector = LocalCollector::start();

                        four_spans();

                        let local_spans = Arc::new(local_collector.collect());
                        merged.mount_local_spans(local_spans);
                    });
                }

                four_spans();

                let local_spans = Arc::new(local_collector.collect());
                root_span1.mount_local_spans(local_spans.clone());
                root_span2.mount_local_spans(local_spans);
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
                let (root_span1, collector1) = Span::root("root1".to_owned());
                let (root_span2, collector2) = Span::root("root2".to_owned());
                let (root_span3, collector3) = Span::root("root3".to_owned());

                let local_collector = LocalCollector::start();

                let local_spans = Arc::new(local_collector.collect());
                root_span1.mount_local_spans(local_spans.clone());
                root_span2.mount_local_spans(local_spans.clone());
                root_span3.mount_local_spans(local_spans);

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
