// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::Arc;

use minstant::Instant;

use crate::collector::global_collector::Global;
use crate::collector::{Collect, CollectArgs, Collector, ParentSpan, SpanSet};
use crate::local::local_span_line::LOCAL_SPAN_STACK;
use crate::local::raw_span::RawSpan;
use crate::local::span_id::{DefaultIdGenerator, SpanId};
use crate::local::{LocalParentGuard, LocalSpans};
use crate::util::{alloc_parent_spans, ParentSpans};

/// A thread-safe span.
#[must_use]
#[derive(Debug)]
pub struct Span<C: Collect = Global> {
    pub(crate) inner: Option<SpanInner>,
    collect: C,
}

#[derive(Debug)]
pub(crate) struct SpanInner {
    pub(crate) raw_span: RawSpan,
    pub(crate) parents: ParentSpans,
}

impl Span {
    /// Create a place-holder span that never starts recording.
    #[inline]
    pub fn new_noop() -> Self {
        Self::_new_noop()
    }

    pub fn root(event: &'static str) -> (Self, Collector) {
        Self::_root(event)
    }

    pub fn root_with_args(event: &'static str, args: CollectArgs) -> (Self, Collector) {
        Self::_root_with_args(event, args)
    }

    #[inline]
    pub fn enter_with_parent(event: &'static str, parent: &Span) -> Self {
        Self::_enter_with_parent(event, parent)
    }

    #[inline]
    pub fn enter_with_parents<'a>(
        event: &'static str,
        parents: impl IntoIterator<Item = &'a Span>,
    ) -> Self {
        Self::_enter_with_parents(event, parents)
    }

    #[inline]
    pub fn enter_with_local_parent(event: &'static str) -> Self {
        Self::_enter_with_local_parent(event)
    }
}

impl<C: Collect> Span<C> {
    #[inline]
    pub(crate) fn new(parents: ParentSpans, event: &'static str) -> Self {
        let span_id = DefaultIdGenerator::next_id();
        let begin_instant = Instant::now();
        let raw_span = RawSpan::begin_with(span_id, SpanId::new(0), begin_instant, event);

        Self {
            inner: Some(SpanInner { raw_span, parents }),
            collect: C::default(),
        }
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
    pub fn set_local_parent(&self) -> LocalParentGuard<C> {
        LocalParentGuard::<C>::new(self)
    }

    #[inline]
    pub fn push_child_spans(&self, local_spans: Arc<LocalSpans>) {
        if let Some(inner) = &self.inner {
            let mut parents = alloc_parent_spans();
            parents.extend(inner.as_parent());
            self.collect
                .submit_spans(SpanSet::SharedLocalSpans(local_spans), parents);
        }
    }
}

impl SpanInner {
    #[inline]
    pub(crate) fn as_parent(&self) -> impl Iterator<Item = ParentSpan> + '_ {
        self.parents
            .iter()
            .map(move |ParentSpan { collect_id, .. }| ParentSpan {
                span_id: self.raw_span.id,
                collect_id: *collect_id,
            })
    }
}

impl<C: Collect> Span<C> {
    #[inline]
    pub(crate) fn _new_noop() -> Self {
        Self {
            inner: None,
            collect: C::default(),
        }
    }

    pub(crate) fn _root(event: &'static str) -> (Self, Collector<C>) {
        Self::_root_with_args(event, CollectArgs::default())
    }

    pub(crate) fn _root_with_args(event: &'static str, args: CollectArgs) -> (Self, Collector<C>) {
        let (collector, collect_id) = Collector::start_collect(args);
        let parent = ParentSpan {
            span_id: SpanId::new(0),
            collect_id,
        };
        let mut parents = alloc_parent_spans();
        parents.push(parent);
        let span = Self::new(parents, event);

        (span, collector)
    }

    #[inline]
    pub(crate) fn _enter_with_parent(event: &'static str, parent: &Span<C>) -> Self {
        Self::_enter_with_parents(event, [parent])
    }

    #[inline]
    pub(crate) fn _enter_with_parents<'a>(
        event: &'static str,
        parents: impl IntoIterator<Item = &'a Span<C>>,
    ) -> Self {
        let mut parents_spans = alloc_parent_spans();
        parents_spans.extend(
            parents
                .into_iter()
                .filter_map(|span| span.inner.as_ref())
                .flat_map(|inner| inner.as_parent()),
        );

        Self::new(parents_spans, event)
    }

    #[inline]
    pub(crate) fn _enter_with_local_parent(event: &'static str) -> Self {
        LOCAL_SPAN_STACK
            .with(|span_stack| {
                let mut span_stack = span_stack.borrow_mut();
                let parents = span_stack.current_span_line()?.current_parents()?;
                Some(Span::new(parents, event))
            })
            .unwrap_or_else(Self::_new_noop)
    }
}

impl<C: Collect> Drop for Span<C> {
    fn drop(&mut self) {
        if let Some(mut inner) = self.inner.take() {
            let end_instant = Instant::now();
            inner.raw_span.end_with(end_instant);
            self.collect
                .submit_spans(SpanSet::Span(inner.raw_span), inner.parents);
        }
    }
}
