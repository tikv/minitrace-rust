// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;

use minstant::Instant;

use crate::collector::global_collector::SpanSet;
use crate::collector::{global_collector, ParentSpan};
use crate::local::local_collector::LocalCollector;
use crate::local::span_queue::{SpanHandle, SpanQueue};
use crate::local::LocalSpans;
use crate::util::{alloc_parent_spans, ParentSpans, RawSpans};

thread_local! {
    pub(crate) static LOCAL_SPAN_STACK: RefCell<LocalSpanStack> = RefCell::new(LocalSpanStack::new());
}

pub(crate) struct LocalSpanStack {
    span_lines: Vec<SpanLine>,
    next_local_collector_epoch: usize,
}

pub(crate) struct SpanLine {
    span_queue: SpanQueue,
    local_collector_epoch: usize,
    parents: Option<ParentSpans>,
}

pub(crate) struct LocalSpanHandle {
    local_collector_epoch: usize,
    span_handle: SpanHandle,
}

impl LocalSpanStack {
    #[inline]
    pub fn new() -> Self {
        Self {
            span_lines: Vec::new(),
            next_local_collector_epoch: 0,
        }
    }

    #[inline]
    pub fn current_span_line(&mut self) -> Option<&mut SpanLine> {
        self.span_lines.last_mut()
    }

    #[inline]
    pub fn enter_span(&mut self, event: &'static str) -> Option<LocalSpanHandle> {
        let span_line = self.current_span_line()?;

        Some(LocalSpanHandle {
            span_handle: span_line.span_queue.start_span(event),
            local_collector_epoch: span_line.local_collector_epoch,
        })
    }

    #[inline]
    pub fn exit_span(&mut self, local_span_handle: LocalSpanHandle) {
        if let Some(span_line) = self.current_span_line() {
            debug_assert_eq!(
                span_line.local_collector_epoch,
                local_span_handle.local_collector_epoch
            );

            if span_line.local_collector_epoch == local_span_handle.local_collector_epoch {
                span_line
                    .span_queue
                    .finish_span(local_span_handle.span_handle);
            }
        }
    }

    #[inline]
    pub fn register_local_collector(&mut self, parents: Option<ParentSpans>) -> LocalCollector {
        let epoch = self.next_local_collector_epoch;
        self.next_local_collector_epoch = self.next_local_collector_epoch.wrapping_add(1);

        let span_line = SpanLine {
            span_queue: SpanQueue::new(),
            local_collector_epoch: epoch,
            parents,
        };

        self.span_lines.push(span_line);

        LocalCollector::new(epoch)
    }

    // Raw spans will be sent to acquirers directly and return None if parent span exists.
    pub fn unregister_and_collect(&mut self, local_collector: &LocalCollector) -> Option<RawSpans> {
        debug_assert_eq!(
            self.current_span_line()
                .map(|span_line| span_line.local_collector_epoch),
            Some(local_collector.local_collector_epoch)
        );

        let mut span_line = self.span_lines.pop()?;
        if span_line.local_collector_epoch == local_collector.local_collector_epoch {
            let raw_spans = span_line.span_queue.take_queue();

            if let Some(parents) = span_line.parents.take() {
                let local_spans = LocalSpans {
                    spans: raw_spans,
                    end_time: Instant::now(),
                };
                global_collector::submit_spans(SpanSet::LocalSpans(local_spans), parents);
                None
            } else {
                Some(raw_spans)
            }
        } else {
            None
        }
    }

    #[inline]
    pub fn add_properties<I, F>(&mut self, local_span_handle: &LocalSpanHandle, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        debug_assert!(self.current_span_line().is_some());

        if let Some(span_line) = self.current_span_line() {
            debug_assert_eq!(
                span_line.local_collector_epoch,
                local_span_handle.local_collector_epoch
            );

            if span_line.local_collector_epoch == local_span_handle.local_collector_epoch {
                span_line
                    .span_queue
                    .add_properties(&local_span_handle.span_handle, properties());
            }
        }
    }
}

impl SpanLine {
    #[inline]
    pub fn current_parents<'a>(&'a self) -> Option<ParentSpans> {
        self.parents.as_ref().map(|parents| {
            let mut parents_spans = alloc_parent_spans();
            parents_spans.extend(parents.iter().map(|parent| ParentSpan {
                parent_id: self.span_queue.next_parent_id.unwrap_or(parent.parent_id),
                collect_id: parent.collect_id,
            }));
            parents_spans
        })
    }
}
