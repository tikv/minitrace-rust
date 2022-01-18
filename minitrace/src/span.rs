// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::global_collector::Global;
use crate::collector::{Collect, CollectArgs, CollectTokenItem, Collector, SpanSet};
use crate::local::local_span_stack::{LocalSpanStack, LOCAL_SPAN_STACK};
use crate::local::raw_span::RawSpan;
use crate::local::span_id::{DefaultIdGenerator, SpanId};
use crate::local::{LocalCollector, LocalSpans};
use crate::util::guard::Guard;
use crate::util::{alloc_collect_token, CollectToken};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use minstant::Instant;

/// A thread-safe span.
#[must_use]
#[derive(Debug)]
pub struct Span<C: Collect = Global> {
    pub(crate) inner: Option<SpanInner<C>>,
}

#[derive(Debug)]
pub(crate) struct SpanInner<C: Collect> {
    pub(crate) raw_span: RawSpan,
    pub(crate) collect_token: CollectToken,
    collect: C,
}

impl Span {
    /// Create a place-holder span that never starts recording.
    #[inline]
    pub fn new_noop() -> Self {
        Self::new_noop_with_collect()
    }

    #[inline]
    pub fn root(event: &'static str) -> (Self, Collector<Global>) {
        Self::root_with_args_collect(event, CollectArgs::default(), Global)
    }

    #[inline]
    pub fn root_with_args(event: &'static str, args: CollectArgs) -> (Self, Collector<Global>) {
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
    pub fn set_local_parent(&self) -> Option<Guard<impl FnOnce()>> {
        self.inner.as_ref().map(move |inner| {
            let stack = LOCAL_SPAN_STACK.with(Rc::clone);
            inner.capture_local_spans(stack)
        })
    }

    #[inline]
    pub fn with_property<F>(&mut self, property: F)
    where
        F: FnOnce() -> (&'static str, String),
    {
        self.with_properties(move || [property()]);
    }

    #[inline]
    pub fn with_properties<I, F>(&mut self, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        if let Some(inner) = self.inner.as_mut() {
            inner.with_properties(properties);
        }
    }

    #[inline]
    pub fn push_child_spans(&self, local_spans: Arc<LocalSpans>) {
        if let Some(inner) = self.inner.as_ref() {
            inner.push_child_spans(local_spans)
        }
    }
}

impl<C: Collect> Span<C> {
    #[inline]
    fn new(collect_token: CollectToken, event: &'static str, collect: C) -> Self {
        let span_id = DefaultIdGenerator::next_id();
        let begin_instant = Instant::now();
        let raw_span = RawSpan::begin_with(span_id, SpanId::default(), begin_instant, event);

        Self {
            inner: Some(SpanInner {
                raw_span,
                collect_token,
                collect,
            }),
        }
    }
}

impl<C: Collect> SpanInner<C> {
    #[inline]
    fn with_properties<I, F>(&mut self, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        for prop in properties() {
            self.raw_span.properties.push(prop);
        }
    }

    #[inline]
    fn capture_local_spans(&self, stack: Rc<RefCell<LocalSpanStack>>) -> Guard<impl FnOnce()> {
        let collector = self.register_local_collector(stack);
        let collect = self.collect.clone();
        Guard::new(move || {
            let (spans, token) = collector.collect_with_token();
            debug_assert!(token.is_some());
            let token = token.unwrap_or_else(alloc_collect_token);
            collect.submit_spans(SpanSet::LocalSpans(spans), token);
        })
    }

    #[inline]
    fn register_local_collector(&self, stack: Rc<RefCell<LocalSpanStack>>) -> LocalCollector {
        let mut token = alloc_collect_token();
        token.extend(self.as_collect_token());
        LocalCollector::new(Some(token), stack)
    }

    #[inline]
    fn push_child_spans(&self, local_spans: Arc<LocalSpans>) {
        let mut token = alloc_collect_token();
        token.extend(self.as_collect_token());
        self.collect
            .submit_spans(SpanSet::SharedLocalSpans(local_spans), token);
    }

    #[inline]
    fn as_collect_token(&self) -> impl Iterator<Item = CollectTokenItem> + '_ {
        self.collect_token.iter().map(
            move |CollectTokenItem { collect_id, .. }| CollectTokenItem {
                parent_id_of_roots: self.raw_span.id,
                collect_id: *collect_id,
            },
        )
    }
}

impl<C: Collect> Span<C> {
    #[inline]
    pub(crate) fn new_noop_with_collect() -> Self {
        Self { inner: None }
    }

    pub(crate) fn root_with_args_collect(
        event: &'static str,
        args: CollectArgs,
        collect: C,
    ) -> (Self, Collector<C>) {
        let (collector, collect_id) = Collector::start_collect(args, collect.clone());
        let root_collect_token = CollectTokenItem {
            parent_id_of_roots: SpanId::default(),
            collect_id,
        };
        let mut token = alloc_collect_token();
        token.push(root_collect_token);
        let span = Self::new(token, event, collect);

        (span, collector)
    }

    pub(crate) fn enter_with_parents_collect<'a>(
        event: &'static str,
        parents: impl IntoIterator<Item = &'a Span<C>>,
        collect: C,
    ) -> Self {
        let mut token = alloc_collect_token();
        token.extend(
            parents
                .into_iter()
                .filter_map(|span| span.inner.as_ref())
                .flat_map(|inner| inner.as_collect_token()),
        );

        Self::new(token, event, collect)
    }

    pub(crate) fn enter_with_stack_collect(
        event: &'static str,
        stack: Rc<RefCell<LocalSpanStack>>,
        collect: C,
    ) -> Self {
        let token = {
            let span_stack = &mut *stack.borrow_mut();
            span_stack.current_collect_token()
        };

        match token {
            Some(token) => Span::new(token, event, collect),
            None => Self::new_noop_with_collect(),
        }
    }
}

impl<C: Collect> Drop for Span<C> {
    fn drop(&mut self) {
        if let Some(SpanInner {
            mut raw_span,
            collect_token,
            collect,
        }) = self.inner.take()
        {
            let end_instant = Instant::now();
            raw_span.end_with(end_instant);
            collect.submit_spans(SpanSet::Span(raw_span), collect_token);
        }
    }
}
