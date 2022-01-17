// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::global_collector::Global;
use crate::collector::{Collect, SpanSet};
use crate::local::local_span_line::{LocalSpanStack, LOCAL_SPAN_STACK};
use crate::util::{alloc_raw_spans, ParentSpans, RawSpans};

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
/// let _span1 = LocalSpan::enter_with_local_parent("a child span");
/// drop(_span1);
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
pub struct LocalCollector<C: Collect = Global> {
    inner: Option<LocalCollectorInner>,
    collect: C,
}

#[derive(Debug)]
struct LocalCollectorInner {
    stack: Rc<RefCell<LocalSpanStack>>,
    span_line_epoch: usize,
}

#[derive(Debug)]
pub struct LocalSpans {
    pub(crate) spans: RawSpans,
    pub(crate) end_time: Instant,
}

impl LocalCollector {
    pub fn start() -> Self {
        let stack = LOCAL_SPAN_STACK.with(Rc::clone);
        Self::new(stack, None, Global)
    }
}

impl<C: Collect> LocalCollector<C> {
    pub(crate) fn new(
        stack: Rc<RefCell<LocalSpanStack>>,
        parents: Option<ParentSpans>,
        collect: C,
    ) -> Self {
        let span_line_epoch = {
            let stack = &mut (*stack).borrow_mut();
            stack.register_span_line(parents)
        };

        Self {
            inner: span_line_epoch.map(move |span_line_epoch| LocalCollectorInner {
                stack,
                span_line_epoch,
            }),
            collect,
        }
    }

    pub fn collect(mut self) -> LocalSpans {
        let spans = self
            .inner
            .take()
            .map(
                |LocalCollectorInner {
                     stack,
                     span_line_epoch,
                     ..
                 }| {
                    let s = &mut (*stack).borrow_mut();
                    s.unregister_and_collect(span_line_epoch)
                        .map(|(spans, _)| spans)
                },
            )
            .flatten()
            .unwrap_or_else(alloc_raw_spans);

        LocalSpans {
            spans,
            end_time: Instant::now(),
        }
    }
}

impl<C: Collect> Drop for LocalCollector<C> {
    fn drop(&mut self) {
        if let Some(LocalCollectorInner {
            stack,
            span_line_epoch,
        }) = self.inner.take()
        {
            let s = &mut (*stack).borrow_mut();
            if let Some((spans, Some(parents))) = s.unregister_and_collect(span_line_epoch) {
                self.collect.submit_spans(
                    SpanSet::LocalSpans(LocalSpans {
                        spans,
                        end_time: Instant::now(),
                    }),
                    parents,
                )
            }
        }
    }
}
