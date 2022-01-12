// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::Arc;

use minstant::Instant;

use crate::collector::global_collector::SpanSet;
use crate::collector::{global_collector, Collector, ParentSpan, ParentSpans};
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
    pub(crate) raw_span: RawSpan,
    pub(crate) parents: ParentSpans,
}

impl Span {
    #[inline]
    pub(crate) fn new<'a>(parents: ParentSpans, event: &'static str) -> Self {
        let span_id = DefaultIdGenerator::next_id();
        let begin_instant = Instant::now();
        let raw_span = RawSpan::begin_with(span_id, SpanId::new(0), begin_instant, event);

        Self {
            inner: Some(SpanInner { raw_span, parents }),
        }
    }

    #[inline]
    pub fn new_noop() -> Self {
        Self { inner: None }
    }

    pub fn root(event: &'static str) -> (Self, Collector) {
        let collector = Collector::new();
        let parent = ParentSpan {
            parent_id: SpanId::new(0),
            collect_id: collector.collect_id,
        };
        let span = Self::new(vec![parent], event);
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
                .flat_map(|inner| inner.as_parent())
                .collect(),
            event,
        )
    }

    #[inline]
    pub fn enter_with_local_parent(event: &'static str) -> Self {
        LOCAL_SPAN_STACK
            .with(|span_stack| {
                let mut span_stack = span_stack.borrow_mut();
                let parents = span_stack.current_span_line()?.current_parents()?;
                Some(Span::new(parents, event))
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
                inner.raw_span.properties.push(prop);
            }
        }
    }

    #[inline]
    pub fn set_local_parent(&self) -> LocalParentGuard {
        LocalParentGuard::new(self)
    }

    #[inline]
    pub fn push_child_spans(&self, local_spans: Arc<LocalSpans>) {
        if let Some(inner) = &self.inner {
            global_collector::submit_spans(
                SpanSet::SharedLocalSpans(local_spans),
                inner.as_parent().collect(),
            );
        }
    }
}

impl SpanInner {
    #[inline]
    pub(crate) fn as_parent<'a>(&'a self) -> impl Iterator<Item = ParentSpan> + 'a {
        self.parents
            .iter()
            .map(move |ParentSpan { collect_id, .. }| ParentSpan {
                parent_id: self.raw_span.id,
                collect_id: *collect_id,
            })
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(mut inner) = self.inner.take() {
            let end_instant = Instant::now();
            inner.raw_span.end_with(end_instant);
            global_collector::submit_spans(SpanSet::Span(inner.raw_span), inner.parents);
        }
    }
}
