// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use minstant::Instant;

use crate::collector::CollectTokenItem;
use crate::collector::GlobalCollect;
use crate::collector::SpanContext;
use crate::collector::SpanId;
use crate::collector::SpanSet;
use crate::local::local_collector::LocalSpansInner;
use crate::local::local_span_stack::LocalSpanStack;
use crate::local::local_span_stack::LOCAL_SPAN_STACK;
use crate::local::raw_span::RawSpan;
use crate::local::LocalCollector;
use crate::local::LocalSpans;
use crate::util::CollectToken;

/// A thread-safe span.
#[must_use]
pub struct Span {
    #[cfg(feature = "enable")]
    pub(crate) inner: Option<SpanInner>,
}

pub(crate) struct SpanInner {
    pub(crate) raw_span: RawSpan,
    collect_token: CollectToken,
    // If the span is not a root span, this field is `None`.
    collect_id: Option<usize>,
    collect: GlobalCollect,
}

impl Span {
    /// Create a place-holder span that never starts recording.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let mut root = Span::noop();
    /// ```
    #[inline]
    pub fn noop() -> Self {
        Self {
            #[cfg(feature = "enable")]
            inner: None,
        }
    }

    /// Create a new trace and return its root span.
    ///
    /// Once dropped, the root span automatically submits all associated child spans to the reporter.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let mut root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
    /// ```
    #[inline]
    pub fn root(
        name: &'static str,
        parent: SpanContext,
        #[cfg(test)] collect: GlobalCollect,
    ) -> Self {
        #[cfg(not(feature = "enable"))]
        {
            Self::noop()
        }

        #[cfg(feature = "enable")]
        {
            #[cfg(not(test))]
            let collect = GlobalCollect;
            let collect_id = collect.start_collect();
            let token = CollectTokenItem {
                trace_id: parent.trace_id,
                parent_id: parent.span_id,
                collect_id,
                is_root: true,
            }
            .into();
            Self::new(token, name, Some(collect_id), collect)
        }
    }

    /// Create a new child span associated with the specified parent span.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
    ///
    /// let child = Span::enter_with_parent("child", &root);
    #[inline]
    pub fn enter_with_parent(name: &'static str, parent: &Span) -> Self {
        #[cfg(not(feature = "enable"))]
        {
            Self::noop()
        }

        #[cfg(feature = "enable")]
        {
            match &parent.inner {
                Some(_inner) => Self::enter_with_parents(
                    name,
                    [parent],
                    #[cfg(test)]
                    _inner.collect.clone(),
                ),
                None => Span::noop(),
            }
        }
    }

    /// Create a new child span associated with multiple parent spans.
    ///
    /// This function is particularly useful when a single operation amalgamates multiple requests.
    /// It enables the creation of a unique child span that is interconnected with all the parent spans
    /// related to the requests, thereby obviating the need to generate individual child spans for each parent span.
    ///
    /// The newly created child span, and its children, will have a replica for each trace of parent spans.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let parent1 = Span::root("parent1", SpanContext::new(TraceId(12), SpanId::default()));
    /// let parent2 = Span::root("parent2", SpanContext::new(TraceId(12), SpanId::default()));
    ///
    /// let child = Span::enter_with_parents("child", [&parent1, &parent2]);
    #[inline]
    pub fn enter_with_parents<'a>(
        name: &'static str,
        parents: impl IntoIterator<Item = &'a Span>,
        #[cfg(test)] collect: GlobalCollect,
    ) -> Self {
        #[cfg(not(feature = "enable"))]
        {
            Self::noop()
        }

        #[cfg(feature = "enable")]
        {
            #[cfg(not(test))]
            let collect = GlobalCollect;
            let token = parents
                .into_iter()
                .filter_map(|span| span.inner.as_ref())
                .flat_map(|inner| inner.issue_collect_token())
                .collect();
            Self::new(token, name, None, collect)
        }
    }

    /// Create a new child span associated with the current local span in the current thread.
    ///
    /// If no local span is active, this function returns a no-op span.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
    /// let _g = root.set_local_parent();
    ///
    /// let child = Span::enter_with_local_parent("child");
    /// ```
    #[inline]
    pub fn enter_with_local_parent(
        name: &'static str,
        #[cfg(test)] collect: GlobalCollect,
    ) -> Self {
        #[cfg(not(feature = "enable"))]
        {
            Self::noop()
        }

        #[cfg(feature = "enable")]
        {
            #[cfg(not(test))]
            let collect = GlobalCollect;
            LOCAL_SPAN_STACK.with(move |stack| {
                Self::enter_with_stack(name, &mut (*stack).borrow_mut(), collect)
            })
        }
    }

    /// Sets the current `Span` as the local parent for the current thread.
    ///
    /// This method is used to establish a `Span` as the local parent within the current scope.
    ///
    /// A local parent is necessary for creating a [`LocalSpan`] using [`LocalSpan::enter_with_local_parent()`].
    /// If no local parent is set, `enter_with_local_parent()` will not perform any action.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
    /// let _guard = root.set_local_parent(); // root is now the local parent
    ///
    /// // Now we can create a LocalSpan with root as the local parent.
    /// let _span = LocalSpan::enter_with_local_parent("a child span");
    /// ```
    ///
    /// [`LocalSpan`]: crate::local::LocalSpan
    /// [`LocalSpan::enter_with_local_parent()`]: crate::local::LocalSpan::enter_with_local_parent
    pub fn set_local_parent(&self) -> Option<impl Drop> {
        #[cfg(not(feature = "enable"))]
        {
            None::<Span>
        }

        #[cfg(feature = "enable")]
        {
            LOCAL_SPAN_STACK.with(|s| self.attach_into_stack(s))
        }
    }

    /// Add a single property to the `Span` and return the modified `Span`.
    ///
    /// A property is an arbitrary key-value pair associated with a span.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()))
    ///     .with_property(|| ("key".to_string(), "value".to_string()));
    /// ```
    #[inline]
    pub fn with_property<F>(self, property: F) -> Self
    where F: FnOnce() -> (String, String) {
        self.with_properties(move || [property()])
    }

    /// Add multiple properties to the `Span` and return the modified `Span`.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()))
    ///     .with_properties(|| {
    ///         vec![
    ///             ("key1".to_string(), "value1".to_string()),
    ///             ("key2".to_string(), "value2".to_string()),
    ///         ]
    ///     });
    /// ```
    #[inline]
    pub fn with_properties<I, F>(mut self, properties: F) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
        F: FnOnce() -> I,
    {
        #[cfg(feature = "enable")]
        if let Some(inner) = self.inner.as_mut() {
            inner.add_properties(properties);
        }

        self
    }

    /// Attach a collection of [`LocalSpan`] instances as child spans to the current span.
    ///
    /// This method allows you to associate previously collected `LocalSpan` instances with the current span.
    /// This is particularly useful when the `LocalSpan` instances were initiated before their parent span,
    /// and were collected manually using a [`LocalCollector`].
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::local::LocalCollector;
    /// use minitrace::prelude::*;
    ///
    /// // Collect local spans manually without a parent
    /// let collector = LocalCollector::start();
    /// let span = LocalSpan::enter_with_local_parent("a child span");
    /// drop(span);
    /// let local_spans = collector.collect();
    ///
    /// // Attach the local spans to a parent
    /// let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
    /// root.push_child_spans(local_spans);
    /// ```
    ///
    /// [`LocalSpan`]: crate::local::LocalSpan
    /// [`LocalSpans`]: crate::local::LocalSpans
    /// [`LocalCollector`]: crate::local::LocalCollector
    #[inline]
    pub fn push_child_spans(&self, local_spans: LocalSpans) {
        #[cfg(feature = "enable")]
        {
            if let Some(inner) = self.inner.as_ref() {
                inner.push_child_spans(local_spans.inner)
            }
        }
    }

    /// Dismisses the trace, preventing the reporting of any span records associated with it.
    ///
    /// This is particularly useful when focusing on the tail latency of a program. For instant,
    /// you can dismiss all traces finishes within the 99th percentile.
    ///
    /// # Note
    ///
    /// This method only dismisses the entire trace when called on the root span.
    /// If called on a non-root span, it will only cancel the reporting of that specific span.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let mut root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
    ///
    /// // ..
    #[inline]
    pub fn cancel(&mut self) {
        #[cfg(feature = "enable")]
        if let Some(inner) = self.inner.take() {
            if let Some(collect_id) = inner.collect_id {
                inner.collect.drop_collect(collect_id);
            }
        }
    }
}

#[cfg(feature = "enable")]
impl Span {
    #[inline]
    fn new(
        collect_token: CollectToken,
        name: &'static str,
        collect_id: Option<usize>,
        collect: GlobalCollect,
    ) -> Self {
        let span_id = SpanId::next_id();
        let begin_instant = Instant::now();
        let raw_span = RawSpan::begin_with(span_id, SpanId::default(), begin_instant, name, false);

        Self {
            inner: Some(SpanInner {
                raw_span,
                collect_token,
                collect_id,
                collect,
            }),
        }
    }

    pub(crate) fn enter_with_stack(
        name: &'static str,
        stack: &mut LocalSpanStack,
        collect: GlobalCollect,
    ) -> Self {
        match stack.current_collect_token() {
            Some(token) => Span::new(token, name, None, collect),
            None => Self::noop(),
        }
    }

    pub(crate) fn attach_into_stack(
        &self,
        stack: &Rc<RefCell<LocalSpanStack>>,
    ) -> Option<impl Drop> {
        self.inner
            .as_ref()
            .map(move |inner| inner.capture_local_spans(stack.clone()))
    }
}

#[cfg(feature = "enable")]
impl SpanInner {
    #[inline]
    fn add_properties<I, F>(&mut self, properties: F)
    where
        I: IntoIterator<Item = (String, String)>,
        F: FnOnce() -> I,
    {
        for prop in properties() {
            self.raw_span.properties.push(prop);
        }
    }

    #[inline]
    fn capture_local_spans(&self, stack: Rc<RefCell<LocalSpanStack>>) -> impl Drop {
        let token = self.issue_collect_token().collect();
        let collector = LocalCollector::new(Some(token), stack);
        let collect = self.collect.clone();
        defer::defer(move || {
            let (spans, token) = collector.collect_spans_and_token();
            debug_assert!(token.is_some());
            let token = token.unwrap_or_else(|| [].iter().collect());

            if !spans.spans.is_empty() {
                collect.submit_spans(SpanSet::LocalSpansInner(spans), token);
            }
        })
    }

    #[inline]
    fn push_child_spans(&self, local_spans: Arc<LocalSpansInner>) {
        if local_spans.spans.is_empty() {
            return;
        }

        self.collect.submit_spans(
            SpanSet::SharedLocalSpans(local_spans),
            self.issue_collect_token().collect(),
        );
    }

    #[inline]
    pub(crate) fn issue_collect_token(&self) -> impl Iterator<Item = CollectTokenItem> + '_ {
        self.collect_token
            .iter()
            .map(move |collect_item| CollectTokenItem {
                trace_id: collect_item.trace_id,
                parent_id: self.raw_span.id,
                collect_id: collect_item.collect_id,
                is_root: false,
            })
    }

    #[inline]
    pub(crate) fn submit_spans(self) {
        self.collect
            .submit_spans(SpanSet::Span(self.raw_span), self.collect_token);
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        #[cfg(feature = "enable")]
        if let Some(mut inner) = self.inner.take() {
            let collect_id = inner.collect_id.take();
            let collect = inner.collect.clone();

            let end_instant = Instant::now();
            inner.raw_span.end_with(end_instant);
            inner.submit_spans();

            if let Some(collect_id) = collect_id {
                collect.commit_collect(collect_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    use mockall::predicate;
    use mockall::Sequence;
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    use super::*;
    use crate::collector::MockGlobalCollect;
    use crate::local::LocalSpan;
    use crate::prelude::TraceId;
    use crate::util::tree::tree_str_from_span_sets;

    #[test]
    fn noop_basic() {
        let span = Span::noop();
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));
        assert!(span.attach_into_stack(&stack).is_none());
        assert!(stack.borrow_mut().enter_span("span1").is_none());
    }

    #[test]
    fn collect_root() {
        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .return_const(42_usize);
        mock.expect_submit_spans()
            .times(1)
            .in_sequence(&mut seq)
            .with(
                predicate::always(),
                predicate::eq::<CollectToken>(
                    CollectTokenItem {
                        trace_id: TraceId(12),
                        parent_id: SpanId::default(),
                        collect_id: 42,
                        is_root: true,
                    }
                    .into(),
                ),
            )
            .return_const(());
        mock.expect_commit_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(42_usize))
            .return_const(());
        mock.expect_drop_collect().times(0);

        let mock: Arc<MockGlobalCollect> = Arc::new(mock);
        let _root = Span::root(
            "root",
            SpanContext::new(TraceId(12), SpanId::default()),
            mock,
        );
    }

    #[test]
    fn cancel_root() {
        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .return_const(42_usize);
        mock.expect_drop_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(42_usize))
            .return_const(());
        mock.expect_commit_collect().times(0);
        mock.expect_submit_spans().times(0);

        let mock = Arc::new(mock);
        let mut root = Span::root(
            "root",
            SpanContext::new(TraceId(12), SpanId::default()),
            mock,
        );
        root.cancel();
    }

    #[test]
    fn span_with_parent() {
        let routine = |collect| {
            let parent_ctx = SpanContext::new(TraceId(12), SpanId::default());
            let root = Span::root("root", parent_ctx, collect);
            let child1 = Span::enter_with_parent("child1", &root)
                .with_properties(|| [("k1".to_string(), "v1".to_string())]);
            let grandchild = Span::enter_with_parent("grandchild", &child1);
            let child2 = Span::enter_with_parent("child2", &root);

            crossbeam::scope(move |scope| {
                let mut rng = thread_rng();
                let mut spans = [child1, grandchild, child2];
                spans.shuffle(&mut rng);
                for span in spans {
                    scope.spawn(|_| drop(span));
                }
            })
            .unwrap();
        };

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        let span_sets = Arc::new(Mutex::new(Vec::new()));
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .return_const(42_usize);
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
            .with(predicate::eq(42_usize))
            .return_const(());
        mock.expect_drop_collect().times(0);

        routine(Arc::new(mock));
        let span_sets = std::mem::take(&mut *span_sets.lock().unwrap());
        assert_eq!(
            tree_str_from_span_sets(span_sets.as_slice()),
            r#"
#42
root []
    child1 [("k1", "v1")]
        grandchild []
    child2 []
"#
        );
    }

    #[test]
    fn span_with_parents() {
        let routine = |collect: GlobalCollect| {
            let parent_ctx = SpanContext::new(TraceId(12), SpanId::default());
            let parent1 = Span::root("parent1", parent_ctx, collect.clone());
            let parent2 = Span::root("parent2", parent_ctx, collect.clone());
            let parent3 = Span::root("parent3", parent_ctx, collect.clone());
            let parent4 = Span::root("parent4", parent_ctx, collect.clone());
            let parent5 = Span::root("parent5", parent_ctx, collect.clone());
            let child1 = Span::enter_with_parent("child1", &parent5);
            let child2 = Span::enter_with_parents(
                "child2",
                [&parent1, &parent2, &parent3, &parent4, &parent5, &child1],
                collect,
            )
            .with_property(|| ("k1".to_string(), "v1".to_string()));

            crossbeam::scope(move |scope| {
                let mut rng = thread_rng();
                let mut spans = [child1, child2];
                spans.shuffle(&mut rng);
                for span in spans {
                    scope.spawn(|_| drop(span));
                }
            })
            .unwrap();
            crossbeam::scope(move |scope| {
                let mut rng = thread_rng();
                let mut spans = [parent1, parent2, parent3, parent4, parent5];
                spans.shuffle(&mut rng);
                for span in spans {
                    scope.spawn(|_| drop(span));
                }
            })
            .unwrap();
        };

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        let span_sets = Arc::new(Mutex::new(Vec::new()));
        mock.expect_start_collect()
            .times(5)
            .in_sequence(&mut seq)
            .returning({
                let id = Arc::new(AtomicUsize::new(1));
                move || id.fetch_add(1, Ordering::SeqCst)
            });
        mock.expect_submit_spans()
            .times(7)
            .in_sequence(&mut seq)
            .returning({
                let span_sets = span_sets.clone();
                move |span_set, token| span_sets.lock().unwrap().push((span_set, token))
            });
        mock.expect_commit_collect()
            .times(5)
            .with(predicate::in_iter([1_usize, 2, 3, 4, 5]))
            .return_const(());
        mock.expect_drop_collect().times(0);

        routine(Arc::new(mock));
        let span_sets = std::mem::take(&mut *span_sets.lock().unwrap());
        assert_eq!(
            tree_str_from_span_sets(span_sets.as_slice()),
            r#"
#1
parent1 []
    child2 [("k1", "v1")]

#2
parent2 []
    child2 [("k1", "v1")]

#3
parent3 []
    child2 [("k1", "v1")]

#4
parent4 []
    child2 [("k1", "v1")]

#5
parent5 []
    child1 []
        child2 [("k1", "v1")]
    child2 [("k1", "v1")]
"#
        );
    }

    #[test]
    fn span_push_child_spans() {
        let routine = |collect: GlobalCollect| {
            let parent_ctx = SpanContext::new(TraceId(12), SpanId::default());
            let parent1 = Span::root("parent1", parent_ctx, collect.clone());
            let parent2 = Span::root("parent2", parent_ctx, collect.clone());
            let parent3 = Span::root("parent3", parent_ctx, collect.clone());
            let parent4 = Span::root("parent4", parent_ctx, collect.clone());
            let parent5 = Span::root("parent5", parent_ctx, collect);

            let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));
            let collector = LocalCollector::new(None, stack.clone());
            {
                let _s = LocalSpan::enter_with_stack("child", stack);
            }
            let spans = collector.collect();

            for parent in [&parent1, &parent2, &parent3, &parent4, &parent5] {
                parent.push_child_spans(spans.clone());
            }

            crossbeam::scope(move |scope| {
                let mut rng = thread_rng();
                let mut spans = [parent1, parent2, parent3, parent4, parent5];
                spans.shuffle(&mut rng);
                for span in spans {
                    scope.spawn(|_| drop(span));
                }
            })
            .unwrap();
        };

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        let span_sets = Arc::new(Mutex::new(Vec::new()));
        mock.expect_start_collect()
            .times(5)
            .in_sequence(&mut seq)
            .returning({
                let id = Arc::new(AtomicUsize::new(1));
                move || id.fetch_add(1, Ordering::SeqCst)
            });
        mock.expect_submit_spans()
            .times(10)
            .in_sequence(&mut seq)
            .returning({
                let span_sets = span_sets.clone();
                move |span_set, token| span_sets.lock().unwrap().push((span_set, token))
            });
        mock.expect_commit_collect()
            .times(5)
            .with(predicate::in_iter([1_usize, 2, 3, 4, 5]))
            .return_const(());
        mock.expect_drop_collect().times(0);

        routine(Arc::new(mock));
        let span_sets = std::mem::take(&mut *span_sets.lock().unwrap());
        assert_eq!(
            tree_str_from_span_sets(span_sets.as_slice()),
            r"
#1
parent1 []
    child []

#2
parent2 []
    child []

#3
parent3 []
    child []

#4
parent4 []
    child []

#5
parent5 []
    child []
"
        );
    }

    #[test]
    fn span_communicate_via_stack() {
        let routine = |collect: GlobalCollect| {
            let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));

            {
                let parent_ctx = SpanContext::new(TraceId(12), SpanId::default());
                let root = Span::root("root", parent_ctx, collect.clone());
                let _g = root.attach_into_stack(&stack).unwrap();
                let child =
                    Span::enter_with_stack("child", &mut stack.borrow_mut(), collect.clone());
                {
                    let _g = child.attach_into_stack(&stack).unwrap();
                    let _s = Span::enter_with_stack("grandchild", &mut stack.borrow_mut(), collect);
                }
                let _s = LocalSpan::enter_with_stack("local", stack);
            }
        };

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        let span_sets = Arc::new(Mutex::new(Vec::new()));
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .return_const(42_usize);
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
            .with(predicate::eq(42_usize))
            .return_const(());
        mock.expect_drop_collect().times(0);

        routine(Arc::new(mock));
        let span_sets = std::mem::take(&mut *span_sets.lock().unwrap());
        assert_eq!(
            tree_str_from_span_sets(span_sets.as_slice()),
            r#"
#42
root []
    child []
        grandchild []
    local []
"#
        );
    }
}
