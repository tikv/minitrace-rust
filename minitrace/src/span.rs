// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use minstant::Instant;
use smallvec::SmallVec;

use crate::collector::acquirer::{Acquirer, SpanCollection};
use crate::collector::Collector;
use crate::local::local_span_line::LOCAL_SPAN_STACK;
use crate::local::raw_span::RawSpan;
use crate::local::span_id::{DefaultIdGenerator, SpanId};
use crate::local::{LocalParentGuard, LocalSpans};

#[must_use]
#[derive(Debug)]
pub struct Span {
    pub(crate) inner: Option<SpanInner>,
}

#[derive(Debug)]
pub(crate) struct SpanInner {
    pub(crate) span_id: SpanId,

    // Report `RawSpan` to `Acquirer` when `SpanInner` is dropping
    pub(crate) to_report: SmallVec<[(RawSpan, Acquirer); 1]>,
}

impl Span {
    #[inline]
    pub(crate) fn new<'a>(
        acquirers: impl IntoIterator<Item = (SpanId, &'a Acquirer)>,
        event: &'static str,
    ) -> Self {
        let span_id = DefaultIdGenerator::next_id();
        let begin_instant = Instant::now();

        let mut to_report = SmallVec::new();
        for (parent_span_id, acq) in acquirers {
            if !acq.is_shutdown() {
                to_report.push((
                    RawSpan::begin_with(span_id, parent_span_id, begin_instant, event),
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
    pub fn new_noop() -> Self {
        Self { inner: None }
    }

    pub fn root(event: &'static str) -> (Self, Collector) {
        let (tx, rx) = crossbeam::channel::unbounded();
        let closed = Arc::new(AtomicBool::new(false));
        let acquirer = Acquirer::new(tx, closed.clone());
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
        LOCAL_SPAN_STACK
            .with(|span_line| {
                let mut s = span_line.borrow_mut();
                let span_line = s.current_span_line()?;
                let parent_id = span_line.current_parent_id()?;
                Some(Span::new(
                    span_line
                        .current_acquirers()?
                        .iter()
                        .map(|acq| (parent_id, acq)),
                    event,
                ))
            })
            .unwrap_or_else(Self::new_noop)
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
    pub fn set_local_parent(&self) -> LocalParentGuard {
        LocalParentGuard::new(self)
    }

    #[inline]
    pub fn push_child_spans(&self, local_spans: LocalSpans) {
        if let Some(inner) = &self.inner {
            if inner.to_report.len() == 1 {
                inner.to_report[0].1.submit(SpanCollection::LocalSpans {
                    local_spans,
                    parent_id_of_root: inner.span_id,
                });
            } else {
                let local_spans = Arc::new(local_spans);
                for (_, acq) in &inner.to_report {
                    acq.submit(SpanCollection::SharedLocalSpans {
                        local_spans: local_spans.clone(),
                        parent_id_of_root: inner.span_id,
                    })
                }
            }
        }
    }
}

impl Drop for SpanInner {
    fn drop(&mut self) {
        let end_instant = Instant::now();
        for (mut span, acq) in self.to_report.drain(..) {
            span.end_with(end_instant);
            acq.submit(SpanCollection::Span(span))
        }
    }
}
