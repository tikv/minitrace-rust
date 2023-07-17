// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

//! Collector and the collected spans.

#![cfg_attr(test, allow(dead_code))]

pub(crate) mod command;
mod console_reporter;
pub(crate) mod global_collector;
pub(crate) mod id;
mod test_reporter;

use std::rc::Rc;
use std::sync::Arc;

#[cfg(feature = "report")]
pub use console_reporter::ConsoleReporter;
#[cfg(not(test))]
pub(crate) use global_collector::GlobalCollect;
#[cfg(test)]
pub(crate) use global_collector::MockGlobalCollect;
#[cfg(feature = "report")]
pub use global_collector::Reporter;
pub use id::SpanId;
pub use id::TraceId;
#[doc(hidden)]
pub use test_reporter::TestReporter;

use crate::local::local_span_stack::LOCAL_SPAN_STACK;
use crate::local::raw_span::RawSpan;
use crate::local::LocalSpans;
use crate::util::CollectToken;
use crate::Span;
#[cfg(test)]
pub(crate) type GlobalCollect = Arc<MockGlobalCollect>;

#[doc(hidden)]
#[derive(Debug)]
pub enum SpanSet {
    Span(RawSpan),
    LocalSpans(LocalSpans),
    SharedLocalSpans(Arc<LocalSpans>),
}

/// A span record been collected.
#[derive(Clone, Debug, Default)]
pub struct SpanRecord {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_id: SpanId,
    pub begin_unix_time_ns: u64,
    pub duration_ns: u64,
    pub name: &'static str,
    pub properties: Vec<(&'static str, String)>,
    pub events: Vec<EventRecord>,
}

/// A span record been collected.
#[derive(Clone, Debug, Default)]
pub struct EventRecord {
    pub name: &'static str,
    pub timestamp_unix_ns: u64,
    pub properties: Vec<(&'static str, String)>,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CollectTokenItem {
    pub trace_id: TraceId,
    pub parent_id: SpanId,
    pub collect_id: u32,
}

/// `Collector` collects all spans associated to a root span.
pub(crate) struct Collector {
    collect_id: Option<u32>,
    collect: GlobalCollect,
}

impl Collector {
    pub(crate) fn start_collect(
        parent: SpanContext,
        collect: GlobalCollect,
    ) -> (Self, CollectToken) {
        let collect_id = collect.start_collect();

        (
            Collector {
                collect_id: Some(collect_id),
                collect,
            },
            CollectTokenItem {
                trace_id: parent.trace_id,
                parent_id: parent.span_id,
                collect_id,
            }
            .into(),
        )
    }

    pub(crate) fn collect(mut self) {
        if let Some(collect_id) = self.collect_id.take() {
            self.collect.commit_collect(collect_id);
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

#[derive(Clone, Copy, Debug, Default)]
pub struct SpanContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
}

impl SpanContext {
    pub fn new(trace_id: TraceId, span_id: SpanId) -> Self {
        Self { trace_id, span_id }
    }

    pub fn from_span(span: &Span) -> Option<Self> {
        #[cfg(not(feature = "report"))]
        {
            None
        }

        #[cfg(feature = "report")]
        {
            let inner = span.inner.as_ref()?;
            let collect_token = inner.issue_collect_token().next()?;

            Some(Self {
                trace_id: collect_token.trace_id,
                span_id: collect_token.parent_id,
            })
        }
    }

    pub fn from_local() -> Option<Self> {
        #[cfg(not(feature = "report"))]
        {
            None
        }

        #[cfg(feature = "report")]
        {
            let stack = LOCAL_SPAN_STACK.with(Rc::clone);
            let mut stack = stack.borrow_mut();
            let collect_token = stack.current_collect_token()?[0];

            Some(Self {
                trace_id: collect_token.trace_id,
                span_id: collect_token.parent_id,
            })
        }
    }
}

/// Configuration of the behavior of the global collector.
#[must_use]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub struct Config {
    pub(crate) max_span_count: Option<usize>,
}

impl Config {
    /// A soft limit for the span collection for a trace, usually used to avoid out-of-memory.
    ///
    /// # Notice
    ///
    /// Root span will always be collected. The eventually collected spans may exceed the limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::collector::Config;
    /// use minitrace::prelude::*;
    ///
    /// let config = Config::default().max_span_count(Some(100));
    /// minitrace::set_reporter(minitrace::collector::ConsoleReporter, config);
    /// ```
    pub fn max_span_count(self, max_span_count: Option<usize>) -> Self {
        Self { max_span_count }
    }
}

#[cfg(test)]
mod tests {
    use mockall::predicate;
    use mockall::Sequence;

    use super::*;
    use crate::collector::CollectTokenItem;

    #[test]
    fn collector_basic() {
        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .return_const(42_u32);
        mock.expect_commit_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(42_u32))
            .return_const(());
        mock.expect_submit_spans().times(0);
        mock.expect_drop_collect().times(0);

        let mock = Arc::new(mock);
        let (collector, token) =
            Collector::start_collect(SpanContext::new(TraceId(12), SpanId::default()), mock);
        collector.collect();
        assert_eq!(token.into_inner().1.as_slice(), &[CollectTokenItem {
            trace_id: TraceId(12),
            parent_id: SpanId::default(),
            collect_id: 42
        }]);
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
        let (_collector, token) =
            Collector::start_collect(SpanContext::new(TraceId(12), SpanId::default()), mock);
        assert_eq!(token.into_inner().1.as_slice(), &[CollectTokenItem {
            trace_id: TraceId(12),
            parent_id: SpanId::default(),
            collect_id: 42
        }]);
    }
}
