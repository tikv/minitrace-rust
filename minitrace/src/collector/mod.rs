// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

pub(crate) mod acquirer;

use crossbeam::channel::Receiver;
use lockfree_object_pool::{LinearObjectPool, LinearReusable};
use once_cell::sync::Lazy;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use minstant::Anchor;

use crate::collector::acquirer::SpanCollection;
use crate::local::raw_span::RawSpan;
use crate::local::span_id::SpanId;
use crate::local::LocalSpans;

pub(crate) static RAW_SPAN_VEC_POOL: Lazy<LinearObjectPool<Vec<RawSpan>>> =
    Lazy::new(|| LinearObjectPool::new(Vec::new, Vec::clear));

pub(crate) type RawSpans = LinearReusable<'static, Vec<RawSpan>>;

#[derive(Clone, Debug, Default)]
pub struct SpanRecord {
    pub id: u32,
    pub parent_id: u32,
    pub begin_unix_time_ns: u64,
    pub duration_ns: u64,
    pub event: &'static str,
    pub properties: Vec<(&'static str, String)>,
}

impl SpanRecord {
    #[inline]
    pub(crate) fn from_raw_span(raw_span: RawSpan, anchor: &Anchor) -> SpanRecord {
        let begin_unix_time_ns = raw_span.begin_instant.as_unix_nanos(anchor);
        let end_unix_time_ns = raw_span.end_instant.as_unix_nanos(anchor);
        SpanRecord {
            id: raw_span.id.0,
            parent_id: raw_span.parent_id.0,
            begin_unix_time_ns,
            duration_ns: end_unix_time_ns - begin_unix_time_ns,
            event: raw_span.event,
            properties: raw_span.properties,
        }
    }
}

pub struct Collector {
    receiver: Receiver<SpanCollection>,
    closed: Arc<AtomicBool>,
}

impl Collector {
    pub(crate) fn new(receiver: Receiver<SpanCollection>, closed: Arc<AtomicBool>) -> Self {
        Collector { receiver, closed }
    }

    pub fn collect(self) -> Vec<SpanRecord> {
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
    ) -> Vec<SpanRecord> {
        // TODO: avoid allocation
        let span_collections: Vec<_> = if sync {
            self.receiver.iter().collect()
        } else {
            self.receiver.try_iter().collect()
        };
        self.closed.store(true, Ordering::SeqCst);

        let anchor = Anchor::new();
        if let Some(duration) = duration_threshold {
            // find the root span and check its duration
            if let Some(root_span) = span_collections.iter().find_map(|s| match s {
                SpanCollection::Span(s) if s.parent_id.0 == 0 => Some(s),
                _ => None,
            }) {
                let root_span = SpanRecord::from_raw_span(root_span.clone(), &anchor);
                if root_span.duration_ns < duration.as_nanos() as _ {
                    return vec![root_span];
                }
            }
        }

        Self::amend(span_collections, &anchor)
    }
}

impl Collector {
    #[inline]
    fn amend(span_collections: Vec<SpanCollection>, anchor: &Anchor) -> Vec<SpanRecord> {
        let capacity = span_collections
            .iter()
            .map(|sc| match sc {
                SpanCollection::LocalSpans {
                    local_spans: raw_spans,
                    ..
                } => raw_spans.spans.len(),
                SpanCollection::SharedLocalSpans {
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
                    local_spans,
                    parent_id_of_root,
                } => {
                    Self::amend_local_span(&local_spans, parent_id_of_root, &mut spans, anchor);
                }
                SpanCollection::SharedLocalSpans {
                    local_spans,
                    parent_id_of_root,
                } => {
                    Self::amend_local_span(&*local_spans, parent_id_of_root, &mut spans, anchor);
                }
                SpanCollection::Span(span) => spans.push(SpanRecord::from_raw_span(span, anchor)),
            }
        }

        spans
    }

    fn amend_local_span(
        local_spans: &LocalSpans,
        parent_id_of_root: SpanId,
        spans: &mut Vec<SpanRecord>,
        anchor: &Anchor,
    ) {
        for span in local_spans.spans.iter() {
            let begin_unix_time_ns = span.begin_instant.as_unix_nanos(anchor);
            let end_unix_time_ns = if span.end_instant == span.begin_instant {
                local_spans.end_time.as_unix_nanos(anchor)
            } else {
                span.end_instant.as_unix_nanos(anchor)
            };
            let parent_id = if span.parent_id.0 == 0 {
                parent_id_of_root.0
            } else {
                span.parent_id.0
            };
            spans.push(SpanRecord {
                id: span.id.0,
                parent_id,
                begin_unix_time_ns,
                duration_ns: end_unix_time_ns - begin_unix_time_ns,
                event: span.event,
                properties: span.properties.clone(),
            });
        }
    }
}

#[must_use]
#[derive(Default, Debug)]
pub struct CollectArgs {
    sync: bool,
    duration_threshold: Option<Duration>,
}

impl CollectArgs {
    #[must_use]
    #[allow(clippy::double_must_use)]
    pub fn sync(self, sync: bool) -> Self {
        Self { sync, ..self }
    }

    #[must_use]
    #[allow(clippy::double_must_use)]
    pub fn duration_threshold(self, duration_threshold: Duration) -> Self {
        Self {
            duration_threshold: Some(duration_threshold),
            ..self
        }
    }
}
