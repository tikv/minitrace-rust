// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;

use minstant::Instant;

use crate::{
    local::local_span_line::LOCAL_SPAN_STACK,
    util::{ParentSpans, RawSpans},
};

/// A Collector to collect [`LocalSpan`].
///
/// [`LocalCollector`] allows collect [`LocalSpan`] manully without a local parent. The collected [`LocalSpan`] can later be
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
/// // Collect local spans manully without a parent
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
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct LocalCollector {
    pub(crate) collected: bool,
    pub(crate) local_collector_epoch: usize,

    // Identical to
    // ```
    // impl !Sync for LocalCollector {}
    // impl !Send for LocalCollector {}
    // ```
    //
    // TODO: Replace it once feature `negative_impls` is stable.
    _p: PhantomData<*const ()>,
}

#[derive(Debug)]
pub struct LocalSpans {
    pub(crate) spans: RawSpans,
    pub(crate) end_time: Instant,
}

impl LocalCollector {
    pub(crate) fn new(local_collector_epoch: usize) -> Self {
        Self {
            collected: false,
            local_collector_epoch,
            _p: Default::default(),
        }
    }

    pub fn start() -> Self {
        LOCAL_SPAN_STACK.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            s.register_local_collector(None)
        })
    }

    pub(crate) fn start_with_parent(parent: ParentSpans) -> Self {
        LOCAL_SPAN_STACK.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            s.register_local_collector(Some(parent))
        })
    }

    pub fn collect(mut self) -> LocalSpans {
        LOCAL_SPAN_STACK.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            self.collected = true;
            LocalSpans {
                // This will panic if `LocalCollector` is started by `start_with_parent_span`
                spans: s.unregister_and_collect(&self).unwrap(),
                end_time: Instant::now(),
            }
        })
    }
}

impl Drop for LocalCollector {
    fn drop(&mut self) {
        if !self.collected {
            self.collected = true;
            LOCAL_SPAN_STACK.with(|span_line| {
                let s = &mut *span_line.borrow_mut();
                s.unregister_and_collect(self);
            })
        }
    }
}
