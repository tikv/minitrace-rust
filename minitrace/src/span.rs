// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::GlobalCollect;
use crate::collector::{CollectArgs, CollectTokenItem, Collector, SpanSet};
use crate::local::local_span_stack::{LocalSpanStack, LOCAL_SPAN_STACK};
use crate::local::raw_span::RawSpan;
use crate::local::span_id::{DefaultIdGenerator, SpanId};
use crate::local::Guard;
use crate::local::{LocalCollector, LocalSpans};
use crate::util::CollectToken;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use minstant::Instant;

/// A thread-safe span.
#[must_use]
#[derive(Debug)]
pub struct Span {
    pub(crate) inner: Option<SpanInner>,
}

#[derive(Debug)]
pub(crate) struct SpanInner {
    pub(crate) raw_span: RawSpan,
    pub(crate) collect_token: CollectToken,
    collect: GlobalCollect,
}

impl Span {
    /// Create a place-holder span that never starts recording.
    #[inline]
    pub fn new_noop() -> Self {
        Self { inner: None }
    }

    #[inline]
    pub fn root(event: &'static str, #[cfg(test)] collect: GlobalCollect) -> (Self, Collector) {
        Self::root_with_args(
            event,
            CollectArgs::default(),
            #[cfg(test)]
            collect,
        )
    }

    #[inline]
    pub fn root_with_args(
        event: &'static str,
        args: CollectArgs,
        #[cfg(test)] collect: GlobalCollect,
    ) -> (Self, Collector) {
        #[cfg(not(test))]
        let collect = GlobalCollect::default();
        let (collector, token) = Collector::start_collect(args, collect.clone());
        let span = Self::new(token, event, collect);
        (span, collector)
    }

    #[inline]
    pub fn enter_with_parent(event: &'static str, parent: &Span) -> Self {
        match &parent.inner {
            Some(_inner) => Self::enter_with_parents(
                event,
                [parent],
                #[cfg(test)]
                _inner.collect.clone(),
            ),
            None => Span::new_noop(),
        }
    }

    #[inline]
    pub fn enter_with_parents<'a>(
        event: &'static str,
        parents: impl IntoIterator<Item = &'a Span>,
        #[cfg(test)] collect: GlobalCollect,
    ) -> Self {
        #[cfg(not(test))]
        let collect = GlobalCollect::default();
        let token = parents
            .into_iter()
            .filter_map(|span| span.inner.as_ref())
            .flat_map(|inner| inner.issue_collect_token())
            .into();
        Self::new(token, event, collect)
    }

    #[inline]
    pub fn enter_with_local_parent(
        event: &'static str,
        #[cfg(test)] collect: GlobalCollect,
    ) -> Self {
        #[cfg(not(test))]
        let collect = GlobalCollect::default();
        LOCAL_SPAN_STACK
            .with(move |stack| Self::enter_with_stack(event, &mut (*stack).borrow_mut(), collect))
    }

    pub fn set_local_parent(&self) -> Option<Guard<impl FnOnce()>> {
        LOCAL_SPAN_STACK.with(|s| self.attach_into_stack(s))
    }

    #[inline]
    pub fn add_property<F>(&mut self, property: F)
    where
        F: FnOnce() -> (&'static str, String),
    {
        self.add_properties(move || [property()]);
    }

    #[inline]
    pub fn add_properties<I, F>(&mut self, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        if let Some(inner) = self.inner.as_mut() {
            inner.add_properties(properties);
        }
    }

    #[inline]
    pub fn push_child_spans(&self, local_spans: Arc<LocalSpans>) {
        if local_spans.spans.is_empty() {
            return;
        }

        if let Some(inner) = self.inner.as_ref() {
            inner.push_child_spans(local_spans)
        }
    }
}

impl Span {
    #[inline]
    fn new(collect_token: CollectToken, event: &'static str, collect: GlobalCollect) -> Self {
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

    pub(crate) fn enter_with_stack(
        event: &'static str,
        stack: &mut LocalSpanStack,
        collect: GlobalCollect,
    ) -> Self {
        match stack.current_collect_token() {
            Some(token) => Span::new(token, event, collect),
            None => Self::new_noop(),
        }
    }

    pub(crate) fn attach_into_stack(
        &self,
        stack: &Rc<RefCell<LocalSpanStack>>,
    ) -> Option<Guard<impl FnOnce()>> {
        self.inner
            .as_ref()
            .map(move |inner| inner.capture_local_spans(stack.clone()))
    }
}

impl SpanInner {
    #[inline]
    fn add_properties<I, F>(&mut self, properties: F)
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
        let token = self.issue_collect_token().into();
        let collector = LocalCollector::new(Some(token), stack);
        let collect = self.collect.clone();
        Guard::new(move || {
            let (spans, token) = collector.collect_with_token();
            debug_assert!(token.is_some());
            let token = token.unwrap_or_else(|| None.into());

            if !spans.spans.is_empty() {
                collect.submit_spans(SpanSet::LocalSpans(spans), token);
            }
        })
    }

    #[inline]
    fn push_child_spans(&self, local_spans: Arc<LocalSpans>) {
        self.collect.submit_spans(
            SpanSet::SharedLocalSpans(local_spans),
            self.issue_collect_token().into(),
        );
    }

    #[inline]
    fn issue_collect_token(&self) -> impl Iterator<Item = CollectTokenItem> + '_ {
        self.collect_token.iter().map(
            move |CollectTokenItem { collect_id, .. }| CollectTokenItem {
                parent_id_of_roots: self.raw_span.id,
                collect_id: *collect_id,
            },
        )
    }
}

impl Drop for Span {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::MockGlobalCollect;
    use crate::util::tree::{t, Tree};

    use std::sync::Mutex;

    use futures::executor::block_on;
    use mockall::{predicate, Sequence};
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    #[test]
    fn noop_basic() {
        let span = Span::new_noop();
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));
        assert!(span.attach_into_stack(&stack).is_none());
        assert!(stack.borrow_mut().enter_span("span1").is_none());
    }

    #[test]
    fn span_with_parent() {
        let routine = |collect| {
            let (root_span, root_collector) = Span::root("root", collect);
            let child1 = Span::enter_with_parent("child1", &root_span);
            let grandchild = Span::enter_with_parent("grandchild", &child1);
            let child2 = Span::enter_with_parent("child2", &root_span);

            crossbeam::scope(move |scope| {
                let mut rng = thread_rng();
                let mut spans = [root_span, child1, grandchild, child2];
                spans.shuffle(&mut rng);
                for span in spans {
                    scope.spawn(|_| drop(span));
                }
            })
            .unwrap();

            let _ = block_on(root_collector.collect());
        };

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        let span_sets = Arc::new(Mutex::new(Vec::new()));
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(CollectArgs::default()))
            .return_const(42_u32);
        mock.expect_submit_spans()
            .times(4)
            .in_sequence(&mut seq)
            .withf(|_, collect_token| collect_token.len() == 1 && collect_token[0].collect_id == 42)
            .returning({
                let span_sets = span_sets.clone();
                move |span_set, token| span_sets.lock().unwrap().push((span_set, token))
            });
        mock.expect_commit_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(42_u32))
            .return_const(vec![]);
        mock.expect_drop_collect().times(0);

        routine(Arc::new(mock));
        let span_sets = std::mem::take(&mut *span_sets.lock().unwrap());
        assert_eq!(
            Tree::from_span_sets(span_sets.as_slice()).as_slice(),
            &[(
                42_u32,
                t(
                    "root",
                    [t("child1", [t("grandchild", [])]), t("child2", [])]
                )
            )]
        );
    }
}
