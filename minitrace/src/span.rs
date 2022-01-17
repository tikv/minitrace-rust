// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::global_collector::Global;
use crate::collector::{Collect, CollectArgs, Collector, ParentSpan, SpanSet};
use crate::local::local_span_line::{LocalSpanStack, LOCAL_SPAN_STACK};
use crate::local::raw_span::RawSpan;
use crate::local::span_id::{DefaultIdGenerator, SpanId};
use crate::local::{LocalCollector, LocalSpans};
use crate::util::{alloc_parent_spans, ParentSpans};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use minstant::Instant;

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
        Self::new_noop_with_collect(Global)
    }

    #[inline]
    pub fn root(event: &'static str) -> (Self, Collector) {
        Self::root_with_args_collect(event, CollectArgs::default(), Global)
    }

    #[inline]
    pub fn root_with_args(event: &'static str, args: CollectArgs) -> (Self, Collector) {
        Self::root_with_args_collect(event, args, Global)
    }

    #[inline]
    pub fn enter_with_parent(event: &'static str, parent: &Span) -> Self {
        Self::enter_with_parents_collect(event, [parent], Global)
    }

    #[inline]
    pub fn enter_with_parents<'a>(
        event: &'static str,
        parents: impl IntoIterator<Item = &'a Span>,
    ) -> Self {
        Self::enter_with_parents_collect(event, parents, Global)
    }

    #[inline]
    pub fn enter_with_local_parent(event: &'static str) -> Self {
        let stack = LOCAL_SPAN_STACK.with(Rc::clone);
        Self::enter_with_stack_collect(event, stack, Global)
    }
}

impl<C: Collect> Span<C> {
    #[inline]
    pub(crate) fn new(parents: ParentSpans, event: &'static str, collect: C) -> Self {
        let span_id = DefaultIdGenerator::next_id();
        let begin_instant = Instant::now();
        let raw_span = RawSpan::begin_with(span_id, SpanId::new(0), begin_instant, event);

        Self {
            inner: Some(SpanInner { raw_span, parents }),
            collect,
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
    pub fn set_local_parent(&self) -> Option<LocalCollector<C>> {
        self.as_parents().map(|parents| {
            let stack = LOCAL_SPAN_STACK.with(Rc::clone);
            LocalCollector::new(stack, Some(parents), self.collect.clone())
        })
    }

    #[inline]
    pub fn push_child_spans(&self, local_spans: Arc<LocalSpans>) {
        if let Some(parents) = self.as_parents() {
            self.collect
                .submit_spans(SpanSet::SharedLocalSpans(local_spans), parents);
        }
    }

    #[inline]
    pub(crate) fn as_parents(&self) -> Option<ParentSpans> {
        self.inner.as_ref().map(|inner| {
            let mut parents = alloc_parent_spans();
            parents.extend(inner.as_parents());
            parents
        })
    }
}

impl SpanInner {
    #[inline]
    pub(crate) fn as_parents(&self) -> impl Iterator<Item = ParentSpan> + '_ {
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
    pub(crate) fn new_noop_with_collect(collect: C) -> Self {
        Self {
            inner: None,
            collect,
        }
    }

    pub(crate) fn root_with_args_collect(
        event: &'static str,
        args: CollectArgs,
        collect: C,
    ) -> (Self, Collector<C>) {
        let (collector, collect_id) = Collector::start_collect(args, collect.clone());
        let parent = ParentSpan {
            span_id: SpanId::new(0),
            collect_id,
        };
        let mut parents = alloc_parent_spans();
        parents.push(parent);
        let span = Self::new(parents, event, collect);

        (span, collector)
    }

    pub(crate) fn enter_with_parents_collect<'a>(
        event: &'static str,
        parents: impl IntoIterator<Item = &'a Span<C>>,
        collect: C,
    ) -> Self {
        let mut parents_spans = alloc_parent_spans();
        parents_spans.extend(
            parents
                .into_iter()
                .filter_map(|span| span.inner.as_ref())
                .flat_map(|inner| inner.as_parents()),
        );

        Self::new(parents_spans, event, collect)
    }

    pub(crate) fn enter_with_stack_collect(
        event: &'static str,
        stack: Rc<RefCell<LocalSpanStack>>,
        collect: C,
    ) -> Self {
        let parents = {
            let span_stack = &mut *stack.borrow_mut();
            span_stack
                .current_span_line()
                .map(|span_line| span_line.current_parents())
                .flatten()
        };

        match parents {
            Some(parents) => Span::new(parents, event, collect),
            None => Self::new_noop_with_collect(collect),
        }
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
