// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::iter;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::local::local_collector::LocalSpans;
use crate::span::RawSpan;
use crate::span::{DefaultClock, DefaultIdGenerator, SpanId};
use crate::trace::acquirer::{Acquirer, SpanCollection};
use crate::Collector;

#[must_use]
#[derive(Debug)]
pub struct Span {
    pub(crate) inner: Option<SpanInner>,
}

#[derive(Debug)]
pub(crate) struct SpanInner {
    pub(crate) span_id: SpanId,

    // Report `RawSpan` to `Acquirer` when `SpanInner` is dropping
    pub(crate) to_report: Vec<(RawSpan, Acquirer)>,
}

impl Span {
    #[inline]
    pub(crate) fn new<'a>(
        acquirers: impl Iterator<Item = (SpanId, &'a Acquirer)>,
        event: String,
    ) -> Self {
        let span_id = DefaultIdGenerator::next_id();
        let now = DefaultClock::now();

        let mut to_report = Vec::new();
        for (parent_span_id, acq) in acquirers {
            if !acq.is_shutdown() {
                to_report.push((
                    RawSpan::begin_with(span_id, parent_span_id, now, event.clone()),
                    acq.clone(),
                ))
            }
        }

        if to_report.is_empty() {
            Self { inner: None }
        } else {
            Self {
                inner: Some(SpanInner { span_id, to_report }),
            }
        }
    }

    pub fn root(event: String) -> (Self, Collector) {
        let (tx, rx) = crossbeam::channel::unbounded();
        let closed = Arc::new(AtomicBool::new(false));
        let acquirer = Acquirer::new(Arc::new(tx), closed.clone());
        let span = Self::new(iter::once((SpanId::new(0), &acquirer)), event);
        let collector = Collector::new(rx, closed);
        (span, collector)
    }

    #[inline]
    pub fn empty() -> Self {
        Self { inner: None }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_none()
    }

    #[inline]
    pub fn from_parent(event: String, span: &Span) -> Self {
        Self::from_parents(event, iter::once(span))
    }

    #[inline]
    pub fn from_parents<'a>(
        event: String,
        spans: impl IntoIterator<Item = &'a Span>,
    ) -> Self {
        Self::new(
            spans
                .into_iter()
                .filter_map(|span| span.inner.as_ref())
                .flat_map(|inner| {
                    inner
                        .to_report
                        .iter()
                        .map(move |(_, acq)| (inner.span_id, acq))
                }),
            event,
        )
    }

    #[inline]
    pub fn mount_local_spans(&self, local_spans: Arc<LocalSpans>) {
        if let Some(inner) = &self.inner {
            for (_, acq) in &inner.to_report {
                acq.submit(SpanCollection::LocalSpans {
                    local_spans: local_spans.clone(),
                    parent_id_of_root: inner.span_id,
                })
            }
        }
    }
}

impl Drop for SpanInner {
    fn drop(&mut self) {
        let now = DefaultClock::now();
        for (mut span, collector) in self.to_report.drain(..) {
            span.end_with(now);
            collector.submit(SpanCollection::Span(span))
        }
    }
}
