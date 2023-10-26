// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

//! Collector and the collected spans.

#![cfg_attr(test, allow(dead_code))]

pub(crate) mod command;
mod console_reporter;
pub(crate) mod global_collector;
pub(crate) mod id;
mod test_reporter;

use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

pub use console_reporter::ConsoleReporter;
#[cfg(not(test))]
pub(crate) use global_collector::GlobalCollect;
#[cfg(test)]
pub(crate) use global_collector::MockGlobalCollect;
pub use global_collector::Reporter;
pub use id::SpanId;
pub use id::TraceId;
#[doc(hidden)]
pub use test_reporter::TestReporter;

use crate::local::local_collector::LocalSpansInner;
use crate::local::local_span_stack::LOCAL_SPAN_STACK;
use crate::local::raw_span::RawSpan;
use crate::Span;
#[cfg(test)]
pub(crate) type GlobalCollect = Arc<MockGlobalCollect>;

#[doc(hidden)]
#[derive(Debug)]
pub enum SpanSet {
    Span(RawSpan),
    LocalSpansInner(LocalSpansInner),
    SharedLocalSpans(Arc<LocalSpansInner>),
}

/// A record of a span that includes all the information about the span,
/// such as its identifiers, timing information, name, and associated properties.
#[derive(Clone, Debug, Default)]
pub struct SpanRecord {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_id: SpanId,
    pub begin_time_unix_ns: u64,
    pub duration_ns: u64,
    pub name: Cow<'static, str>,
    pub properties: Vec<(Cow<'static, str>, Cow<'static, str>)>,
    pub events: Vec<EventRecord>,
}

/// A record of an event that occurred during the execution of a span.
#[derive(Clone, Debug, Default)]
pub struct EventRecord {
    pub name: Cow<'static, str>,
    pub timestamp_unix_ns: u64,
    pub properties: Vec<(Cow<'static, str>, Cow<'static, str>)>,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CollectTokenItem {
    pub trace_id: TraceId,
    pub parent_id: SpanId,
    pub collect_id: usize,
    pub is_root: bool,
}

/// A struct representing the context of a span, including its [`TraceId`] and [`SpanId`].
///
/// [`TraceId`]: crate::collector::TraceId
/// [`SpanId`]: crate::collector::SpanId
#[derive(Clone, Copy, Debug, Default)]
pub struct SpanContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
}

impl SpanContext {
    /// Creates a new `SpanContext` with the given [`TraceId`] and [`SpanId`].
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let context = SpanContext::new(TraceId(12), SpanId::default());
    /// ```
    ///
    /// [`TraceId`]: crate::collector::TraceId
    /// [`SpanId`]: crate::collector::SpanId
    pub fn new(trace_id: TraceId, span_id: SpanId) -> Self {
        Self { trace_id, span_id }
    }

    /// Create a new `SpanContext` with a random trace id.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random());
    /// ```
    pub fn random() -> Self {
        Self {
            trace_id: TraceId(rand::random()),
            span_id: SpanId::default(),
        }
    }

    /// Creates a `SpanContext` from the given [`Span`]. If the `Span` is a noop span,
    /// this function will return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let span = Span::root("root", SpanContext::random());
    /// let context = SpanContext::from_span(&span);
    /// ```
    ///
    /// [`Span`]: crate::Span
    pub fn from_span(span: &Span) -> Option<Self> {
        #[cfg(not(feature = "enable"))]
        {
            None
        }

        #[cfg(feature = "enable")]
        {
            let inner = span.inner.as_ref()?;
            let collect_token = inner.issue_collect_token().next()?;

            Some(Self {
                trace_id: collect_token.trace_id,
                span_id: collect_token.parent_id,
            })
        }
    }

    /// Creates a `SpanContext` from the current local parent span. If there is no
    /// local parent span, this function will return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let span = Span::root("root", SpanContext::random());
    /// let _guard = span.set_local_parent();
    ///
    /// let context = SpanContext::current_local_parent();
    /// ```
    pub fn current_local_parent() -> Option<Self> {
        #[cfg(not(feature = "enable"))]
        {
            None
        }

        #[cfg(feature = "enable")]
        {
            let stack = LOCAL_SPAN_STACK.try_with(Rc::clone).ok()?;

            let mut stack = stack.borrow_mut();
            let collect_token = stack.current_collect_token()?[0];

            Some(Self {
                trace_id: collect_token.trace_id,
                span_id: collect_token.parent_id,
            })
        }
    }

    /// Decodes the `SpanContext` from a [W3C Trace Context](https://www.w3.org/TR/trace-context/)
    /// `traceparent` header string.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let span_context = SpanContext::decode_w3c_traceparent(
    ///     "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(
    ///     span_context.trace_id,
    ///     TraceId(0x0af7651916cd43dd8448eb211c80319c)
    /// );
    /// assert_eq!(span_context.span_id, SpanId(0xb7ad6b7169203331));
    /// ```
    pub fn decode_w3c_traceparent(traceparent: &str) -> Option<Self> {
        let mut parts = traceparent.split('-');

        match (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) {
            (Some("00"), Some(trace_id), Some(span_id), Some(_), None) => {
                let trace_id = u128::from_str_radix(trace_id, 16).ok()?;
                let span_id = u64::from_str_radix(span_id, 16).ok()?;
                Some(Self::new(TraceId(trace_id), SpanId(span_id)))
            }
            _ => None,
        }
    }

    /// Encodes the `SpanContext` into a [W3C Trace Context](https://www.w3.org/TR/trace-context/)
    /// `traceparent` header string.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let span_context = SpanContext::new(TraceId(12), SpanId(34));
    /// let traceparent = span_context.encode_w3c_traceparent();
    ///
    /// assert_eq!(
    ///     traceparent,
    ///     "00-0000000000000000000000000000000c-0000000000000022-01"
    /// );
    /// ```
    pub fn encode_w3c_traceparent(&self) -> String {
        Self::encode_w3c_traceparent_with_sampled(self, true)
    }

    /// Encodes the `SpanContext` as a [W3C Trace Context](https://www.w3.org/TR/trace-context/)
    /// `traceparent` header string with the sampled flag set to false.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let span_context = SpanContext::new(TraceId(12), SpanId(34));
    /// let traceparent = span_context.encode_w3c_traceparent_with_sampled(false);
    ///
    /// assert_eq!(
    ///     traceparent,
    ///     "00-0000000000000000000000000000000c-0000000000000022-00"
    /// );
    /// ```
    pub fn encode_w3c_traceparent_with_sampled(&self, sampled: bool) -> String {
        format!(
            "00-{:032x}-{:016x}-{:02x}",
            self.trace_id.0, self.span_id.0, sampled as u8,
        )
    }
}

/// Configuration of the behavior of the global collector.
#[must_use]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Config {
    pub(crate) max_spans_per_trace: Option<usize>,
    pub(crate) batch_report_interval: Duration,
    pub(crate) batch_report_max_spans: Option<usize>,
}

impl Config {
    /// A soft limit for the total number of spans and events for a trace, usually used
    /// to avoid out-of-memory.
    ///
    /// # Note
    ///
    /// Root span will always be collected. The eventually collected spans may exceed the limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::collector::Config;
    ///
    /// let config = Config::default().max_spans_per_trace(Some(100));
    /// minitrace::set_reporter(minitrace::collector::ConsoleReporter, config);
    /// ```
    pub fn max_spans_per_trace(self, max_spans_per_trace: Option<usize>) -> Self {
        Self {
            max_spans_per_trace,
            ..self
        }
    }

    /// The time duration between two batch reports.
    ///
    /// The default value is 500 milliseconds.
    ///
    /// A batch report will be initiated by the earliest of these events:
    ///
    /// - When the specified time duration between two batch reports is met.
    /// - When the number of spans in a batch hits its limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use minitrace::collector::Config;
    ///
    /// let config = Config::default().batch_report_interval(Duration::from_secs(1));
    /// minitrace::set_reporter(minitrace::collector::ConsoleReporter, config);
    /// ```
    pub fn batch_report_interval(self, batch_report_interval: Duration) -> Self {
        Self {
            batch_report_interval,
            ..self
        }
    }

    /// The soft limit for the maximum number of spans in a batch report.
    ///
    /// A batch report will be initiated by the earliest of these events:
    ///
    /// - When the specified time duration between two batch reports is met.
    /// - When the number of spans in a batch hits its limit.
    ///
    /// # Note
    ///
    /// The eventually spans being reported may exceed the limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use minitrace::collector::Config;
    ///
    /// let config = Config::default().batch_report_max_spans(Some(200));
    /// minitrace::set_reporter(minitrace::collector::ConsoleReporter, config);
    /// ```
    pub fn batch_report_max_spans(self, batch_report_max_spans: Option<usize>) -> Self {
        Self {
            batch_report_max_spans,
            ..self
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_spans_per_trace: None,
            batch_report_interval: Duration::from_millis(500),
            batch_report_max_spans: None,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn w3c_traceparent() {
        let span_context = SpanContext::decode_w3c_traceparent(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
        )
        .unwrap();
        assert_eq!(
            span_context.trace_id,
            TraceId(0x0af7651916cd43dd8448eb211c80319c)
        );
        assert_eq!(span_context.span_id, SpanId(0xb7ad6b7169203331));

        assert_eq!(
            span_context.encode_w3c_traceparent(),
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
        );
        assert_eq!(
            span_context.encode_w3c_traceparent_with_sampled(false),
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00"
        );
    }
}
