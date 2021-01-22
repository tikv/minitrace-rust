// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crossbeam::channel::Receiver;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::span::Span;
use crate::span::{Anchor, DefaultClock};
use crate::trace::acquirer::SpanCollection;

pub struct Collector {
    receiver: Receiver<SpanCollection>,
    closed: Arc<AtomicBool>,
}

impl Collector {
    pub(crate) fn new(receiver: Receiver<SpanCollection>, closed: Arc<AtomicBool>) -> Self {
        Collector { receiver, closed }
    }

    pub fn collect(self) -> Vec<Span> {
        self.collect_with_args(CollectArgs {
            sync: false,
            duration_threshold: None,
        })
    }

    /// Collects spans from traced routines.
    ///
    /// If passing `duration_threshold`, all spans will be reserved only when duration of the root
    /// span exceeds `duration_threshold`, otherwise only one span, the root span, will be returned.
    pub fn collect_with_args(
        self,
        CollectArgs {
            sync,
            duration_threshold,
        }: CollectArgs,
    ) -> Vec<Span> {
        let span_collections: Vec<_> = if sync {
            self.receiver.iter().collect()
        } else {
            self.receiver.try_iter().collect()
        };
        self.closed.store(true, Ordering::SeqCst);

        let anchor = DefaultClock::anchor();
        if let Some(duration) = duration_threshold {
            // find the root span and check its duration
            if let Some(root_span) = span_collections.iter().find_map(|s| match s {
                SpanCollection::Span(s) if s.parent_id.0 == 0 => Some(s),
                _ => None,
            }) {
                let root_span = root_span.build_span(anchor);
                if root_span.duration_ns < duration.as_nanos() as _ {
                    return vec![root_span];
                }
            }
        }

        Self::amend(span_collections, anchor)
    }
}

impl Collector {
    #[inline]
    fn amend(span_collections: Vec<SpanCollection>, anchor: Anchor) -> Vec<Span> {
        let capacity = span_collections
            .iter()
            .map(|sc| match sc {
                SpanCollection::LocalSpans {
                    local_spans: raw_spans,
                    ..
                } => raw_spans.spans.len(),
                SpanCollection::Span(_) => 1,
            })
            .sum();

        let mut spans = Vec::with_capacity(capacity);

        for span_collection in span_collections {
            match span_collection {
                SpanCollection::LocalSpans {
                    local_spans: raw_spans,
                    parent_id_of_root: span_id,
                } => {
                    for span in &raw_spans.spans {
                        let begin_unix_time_ns =
                            DefaultClock::cycle_to_unix_time_ns(span.begin_cycle, anchor);
                        let end_unix_time_ns = if span.end_cycle.is_zero() {
                            DefaultClock::cycle_to_unix_time_ns(raw_spans.end_time, anchor)
                        } else {
                            DefaultClock::cycle_to_unix_time_ns(span.end_cycle, anchor)
                        };
                        let parent_id = if span.parent_id.0 == 0 {
                            span_id.0
                        } else {
                            span.parent_id.0
                        };
                        spans.push(Span {
                            id: span.id.0,
                            parent_id,
                            begin_unix_time_ns,
                            duration_ns: end_unix_time_ns - begin_unix_time_ns,
                            event: span.event,
                            properties: span.properties.clone(),
                        });
                    }
                }
                SpanCollection::Span(span) => spans.push(span.build_span(anchor)),
            }
        }

        spans
    }
}

#[derive(Default, Debug)]
pub struct CollectArgs {
    sync: bool,
    duration_threshold: Option<Duration>,
}

impl CollectArgs {
    pub fn sync(self, sync: bool) -> Self {
        Self { sync, ..self }
    }

    pub fn duration_threshold(self, duration_threshold: Duration) -> Self {
        Self {
            duration_threshold: Some(duration_threshold),
            ..self
        }
    }
}
