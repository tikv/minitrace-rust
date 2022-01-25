// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::local_span_stack::{LocalSpanStack, SpanLineHandle, LOCAL_SPAN_STACK};
use crate::util::{alloc_raw_spans, CollectToken, RawSpans};

use std::cell::RefCell;
use std::rc::Rc;

use minstant::Instant;

/// A collector to collect [`LocalSpan`].
///
/// [`LocalCollector`] allows to collect [`LocalSpan`] manually without a local parent. The collected [`LocalSpan`] can later be
/// mounted to a parent.
///
/// At most time, [`Span`] and [`LocalSpan`] are sufficient. Use [`LocalCollector`] when the span may start before the parent
/// span. Sometimes it is useful to trace the preceding task that is blocking the current request.
///
/// # Examples
///
/// ```
/// use minitrace::prelude::*;
/// use minitrace::local::LocalCollector;
/// use futures::executor::block_on;
/// use std::sync::Arc;
///
/// // Collect local spans manually without a parent
/// let collector = LocalCollector::start();
/// let span = LocalSpan::enter_with_local_parent("a child span");
/// drop(span);
/// let local_spans = collector.collect();
///
/// // Mount the local spans to a parent
/// let (root, collector) = Span::root("root");
/// root.push_child_spans(Arc::new(local_spans));
/// drop(root);
///
/// let records: Vec<SpanRecord> = block_on(collector.collect());
/// ```
///
/// [`Span`]: crate::Span
/// [`LocalSpan`]: crate::local::LocalSpan
#[must_use]
#[derive(Debug, Default)]
pub struct LocalCollector {
    inner: Option<LocalCollectorInner>,
}

#[derive(Debug)]
struct LocalCollectorInner {
    stack: Rc<RefCell<LocalSpanStack>>,
    span_line_handle: SpanLineHandle,
}

#[derive(Debug)]
pub struct LocalSpans {
    pub(crate) spans: RawSpans,
    pub(crate) end_time: Instant,
}

impl LocalCollector {
    pub fn start() -> Self {
        let stack = LOCAL_SPAN_STACK.with(Rc::clone);
        Self::new(None, stack)
    }
}

impl LocalCollector {
    pub(crate) fn new(
        collect_token: Option<CollectToken>,
        stack: Rc<RefCell<LocalSpanStack>>,
    ) -> Self {
        let span_line_epoch = {
            let stack = &mut (*stack).borrow_mut();
            stack.register_span_line(collect_token)
        };

        Self {
            inner: span_line_epoch.map(move |span_line_handle| LocalCollectorInner {
                stack,
                span_line_handle,
            }),
        }
    }

    pub fn collect(self) -> LocalSpans {
        self.collect_spans_and_token().0
    }

    pub(crate) fn collect_spans_and_token(mut self) -> (LocalSpans, Option<CollectToken>) {
        let (spans, collect_token) = self
            .inner
            .take()
            .and_then(
                |LocalCollectorInner {
                     stack,
                     span_line_handle,
                 }| {
                    let s = &mut (*stack).borrow_mut();
                    s.unregister_and_collect(span_line_handle)
                },
            )
            .unwrap_or_else(|| (alloc_raw_spans(), None));

        (
            LocalSpans {
                spans,
                end_time: Instant::now(),
            },
            collect_token,
        )
    }
}

impl Drop for LocalCollector {
    fn drop(&mut self) {
        if let Some(LocalCollectorInner {
            stack,
            span_line_handle,
        }) = self.inner.take()
        {
            let s = &mut (*stack).borrow_mut();
            let _ = s.unregister_and_collect(span_line_handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::CollectTokenItem;
    use crate::local::span_id::SpanId;
    use crate::util::tree::tree_str_from_raw_spans;

    #[test]
    fn local_collector_basic() {
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));
        let collector1 = LocalCollector::new(None, stack.clone());

        let span1 = stack.borrow_mut().enter_span("span1").unwrap();
        {
            let token2 = CollectTokenItem {
                parent_id_of_roots: SpanId::new(9527),
                collect_id: 42,
            };
            let collector2 = LocalCollector::new(Some(token2.into()), stack.clone());
            let span2 = stack.borrow_mut().enter_span("span2").unwrap();
            let span3 = stack.borrow_mut().enter_span("span3").unwrap();
            stack.borrow_mut().exit_span(span3);
            stack.borrow_mut().exit_span(span2);

            let (spans, token) = collector2.collect_spans_and_token();
            assert_eq!(token.unwrap().as_slice(), &[token2]);
            assert_eq!(
                tree_str_from_raw_spans(spans.spans),
                r"
span2 []
    span3 []
"
            );
        }
        stack.borrow_mut().exit_span(span1);
        let spans = collector1.collect();
        assert_eq!(
            tree_str_from_raw_spans(spans.spans),
            r"
span1 []
"
        );
    }

    #[test]
    fn drop_without_collect() {
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));
        let collector1 = LocalCollector::new(None, stack.clone());

        let span1 = stack.borrow_mut().enter_span("span1").unwrap();
        {
            let token2 = CollectTokenItem {
                parent_id_of_roots: SpanId::new(9527),
                collect_id: 42,
            };
            let collector2 = LocalCollector::new(Some(token2.into()), stack.clone());
            let span2 = stack.borrow_mut().enter_span("span2").unwrap();
            let span3 = stack.borrow_mut().enter_span("span3").unwrap();
            stack.borrow_mut().exit_span(span3);
            stack.borrow_mut().exit_span(span2);
            drop(collector2);
        }
        stack.borrow_mut().exit_span(span1);
        let spans = collector1.collect();
        assert_eq!(
            tree_str_from_raw_spans(spans.spans),
            r"
span1 []
"
        );
    }
}
