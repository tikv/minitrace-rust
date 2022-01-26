// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

//! Collector and the collected spans.

pub(crate) mod command;

use crate::local::raw_span::RawSpan;
use crate::local::span_id::SpanId;
use crate::local::LocalSpans;
use crate::util::CollectToken;

use std::sync::Arc;

#[allow(dead_code)]
mod global_collector;
#[cfg(not(test))]
pub(crate) use global_collector::GlobalCollect;
#[cfg(test)]
pub(crate) use global_collector::MockGlobalCollect;
#[cfg(test)]
pub(crate) type GlobalCollect = Arc<MockGlobalCollect>;

pub enum SpanSet {
    Span(RawSpan),
    LocalSpans(LocalSpans),
    SharedLocalSpans(Arc<LocalSpans>),
}

/// A span record been collected.
#[derive(Clone, Debug, Default)]
pub struct SpanRecord {
    pub id: u32,
    pub parent_id: u32,
    pub begin_unix_time_ns: u64,
    pub duration_ns: u64,
    pub event: &'static str,
    pub properties: Vec<(&'static str, String)>,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CollectTokenItem {
    pub parent_id_of_roots: SpanId,
    pub collect_id: u32,
}

/// The collector for collecting all spans of a request.
///
/// A [`Collector`] is provided when starting a root [`Span`](crate::Span) by calling [`Span::root()`](crate::Span::root).
///
/// # Examples
///
/// ```
/// use minitrace::prelude::*;
/// use futures::executor::block_on;
///
/// let (root, collector) = Span::root("root");
/// drop(root);
///
/// let records: Vec<SpanRecord> = block_on(collector.collect());
/// ```
pub struct Collector {
    collect_id: Option<u32>,
    collect: GlobalCollect,
}

impl Collector {
    pub(crate) fn start_collect(args: CollectArgs, collect: GlobalCollect) -> (Self, CollectToken) {
        let collect_id = collect.start_collect(args);

        (
            Collector {
                collect_id: Some(collect_id),
                collect,
            },
            CollectTokenItem {
                parent_id_of_roots: SpanId::default(),
                collect_id,
            }
            .into(),
        )
    }

    /// Stop the trace and collect all span been recorded.
    ///
    /// To extremely eliminate the overhead of tracing, the heavy computation and thread synchronization
    /// work are moved to a background thread, and thus, we have to wait for the background thread to send
    /// the result back. It usually takes 10 milliseconds because the background thread wakes up and processes
    /// messages every 10 milliseconds.
    pub async fn collect(mut self) -> Vec<SpanRecord> {
        match self.collect_id.take() {
            Some(collect_id) => self.collect.commit_collect(collect_id).await,
            None => Vec::default(),
        }
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        if let Some(collect_id) = self.collect_id {
            self.collect.drop_collect(collect_id);
        }
    }
}

/// Arguments for the collector.
///
/// Customize collection behavior through [`Span::root_with_args()`](crate::Span::root_with_args).
#[must_use]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub struct CollectArgs {
    pub(crate) max_span_count: Option<usize>,
}

impl CollectArgs {
    /// A soft limit for the span collection in background, usually used to avoid out-of-memory.
    ///
    /// # Notice
    ///
    /// Root span will always be collected. The eventually collected spans may exceed the limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let args = CollectArgs::default().max_span_count(Some(100));
    /// let (root, collector) = Span::root_with_args("root", args);
    /// ```
    pub fn max_span_count(self, max_span_count: Option<usize>) -> Self {
        Self { max_span_count }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::CollectTokenItem;
    use futures::executor::block_on;
    use mockall::{predicate, Sequence};

    #[test]
    fn collector_basic() {
        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(CollectArgs::default()))
            .return_const(42_u32);
        mock.expect_commit_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(42_u32))
            .return_const(vec![SpanRecord {
                id: 9527,
                event: "span",
                ..SpanRecord::default()
            }]);
        mock.expect_submit_spans().times(0);
        mock.expect_drop_collect().times(0);

        let mock = Arc::new(mock);
        let (collector, token) = Collector::start_collect(CollectArgs::default(), mock);
        assert_eq!(
            token.into_inner().1.as_slice(),
            &[CollectTokenItem {
                parent_id_of_roots: SpanId::default(),
                collect_id: 42
            }]
        );
        let spans = block_on(collector.collect());
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].id, 9527);
        assert_eq!(spans[0].event, "span");
    }

    #[test]
    fn drop_collector() {
        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .return_const(42_u32);
        mock.expect_drop_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(42_u32))
            .return_const(());
        mock.expect_commit_collect().times(0);
        mock.expect_submit_spans().times(0);

        let mock = Arc::new(mock);
        let (_collector, token) = Collector::start_collect(CollectArgs::default(), mock);
        assert_eq!(
            token.into_inner().1.as_slice(),
            &[CollectTokenItem {
                parent_id_of_roots: SpanId::default(),
                collect_id: 42
            }]
        );
    }
}
