// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use minstant::Cycle;

use crate::collector::acquirer::{Acquirer, SpanCollection};
use crate::collector::Collector;
use crate::local::local_parent_guard::AttachedSpan;
use crate::local::raw_span::RawSpan;
use crate::local::span_id::{DefaultIdGenerator, SpanId};
use crate::local::{LocalCollector, LocalParentGuard, LocalSpans};

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
        acquirers: impl IntoIterator<Item = (SpanId, &'a Acquirer)>,
        event: &'static str,
    ) -> Self {
        let span_id = DefaultIdGenerator::next_id();
        let now = Cycle::now();

        let mut to_report = Vec::new();
        for (parent_span_id, acq) in acquirers {
            if !acq.is_shutdown() {
                to_report.push((
                    RawSpan::begin_with(span_id, parent_span_id, now, event),
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

    #[inline]
    pub(crate) fn new_noop() -> Self {
        Self { inner: None }
    }

    pub fn root(event: &'static str) -> (Self, Collector) {
        let (tx, rx) = crossbeam::channel::unbounded();
        let closed = Arc::new(AtomicBool::new(false));
        let acquirer = Acquirer::new(Arc::new(tx), closed.clone());
        let span = Self::new([(SpanId::new(0), &acquirer)], event);
        let collector = Collector::new(rx, closed);
        (span, collector)
    }

    #[inline]
    pub fn enter_with_parent(event: &'static str, parent: &Span) -> Self {
        Self::enter_with_parents(event, [parent])
    }

    #[inline]
    pub fn enter_with_parents<'a>(
        event: &'static str,
        parents: impl IntoIterator<Item = &'a Span>,
    ) -> Self {
        Self::new(
            parents
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
    pub fn enter_with_local_parent(event: &'static str) -> Self {
        AttachedSpan::new_child_span(event).unwrap_or_else(Self::new_noop)
    }

    #[inline]
    pub fn with_property<F>(&mut self, property: F)
    where
        F: FnOnce() -> (&'static str, String),
    {
        self.with_properties(|| [property()]);
    }

    #[inline]
    pub fn with_properties<I, F>(&mut self, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        if let Some(inner) = &mut self.inner {
            for prop in properties() {
                for (raw_span, _) in &mut inner.to_report {
                    raw_span.properties.push(prop.clone());
                }
            }
        }
    }

    #[inline]
    pub fn set_local_parent(&self) -> Option<LocalParentGuard> {
        match LocalCollector::start() {
            Some(local_collector) if !AttachedSpan::is_occupied() => Some(
                LocalParentGuard::new_with_local_collector(self, local_collector),
            ),
            _ => None,
        }
    }

    #[inline]
    pub fn push_child_spans(&self, local_spans: Arc<LocalSpans>) {
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
        let now = Cycle::now();
        for (mut span, collector) in self.to_report.drain(..) {
            span.end_with(now);
            collector.submit(SpanCollection::Span(span))
        }
    }
}
