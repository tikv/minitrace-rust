// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;
use std::rc::Rc;

use crate::collector::ParentSpan;
use crate::local::span_queue::{SpanHandle, SpanQueue};
use crate::util::{alloc_parent_spans, ParentSpans, RawSpans};

const DEFAULT_SPAN_STACK_SIZE: usize = 4096;

thread_local! {
    pub(crate) static LOCAL_SPAN_STACK: Rc<RefCell<LocalSpanStack>> = Rc::new(RefCell::new(LocalSpanStack::with_capacity(DEFAULT_SPAN_STACK_SIZE)));
}

#[derive(Debug)]
pub(crate) struct LocalSpanStack {
    span_lines: Vec<SpanLine>,
    next_span_line_epoch: usize,
}

#[derive(Debug)]
pub(crate) struct SpanLine {
    span_queue: SpanQueue,
    span_line_epoch: usize,
    parents: Option<ParentSpans>,
}

pub(crate) struct LocalSpanHandle {
    span_line_epoch: usize,
    span_handle: SpanHandle,
}

impl LocalSpanStack {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            span_lines: Vec::with_capacity(capacity),
            next_span_line_epoch: 0,
        }
    }

    #[inline]
    pub fn current_span_line(&mut self) -> Option<&mut SpanLine> {
        self.span_lines.last_mut()
    }

    #[inline]
    pub fn enter_span(&mut self, event: &'static str) -> Option<LocalSpanHandle> {
        let span_line = self.current_span_line()?;
        let span_handle = span_line.span_queue.start_span(event)?;

        Some(LocalSpanHandle {
            span_handle,
            span_line_epoch: span_line.span_line_epoch,
        })
    }

    #[inline]
    pub fn exit_span(&mut self, local_span_handle: LocalSpanHandle) {
        if let Some(span_line) = self.current_span_line() {
            debug_assert_eq!(span_line.span_line_epoch, local_span_handle.span_line_epoch);

            if span_line.span_line_epoch == local_span_handle.span_line_epoch {
                span_line
                    .span_queue
                    .finish_span(local_span_handle.span_handle);
            }
        }
    }

    /// Register a new span line to the span stack. If succeed, return a span line epoch which can
    /// be used to unregister the span line via [`LocalSpanStack::unregister_and_collect`]. If
    /// the size of the span stack is greater than the `capacity`, registration will fail
    /// and a `None` will be returned.
    ///
    /// [`LocalSpanStack::unregister_and_collect`](LocalSpanStack::unregister_and_collect)
    #[inline]
    pub(crate) fn register_span_line(&mut self, parents: Option<ParentSpans>) -> Option<usize> {
        if self.span_lines.len() >= self.span_lines.capacity() {
            return None;
        }

        let epoch = self.next_span_line_epoch;
        self.next_span_line_epoch = self.next_span_line_epoch.wrapping_add(1);

        let span_line = SpanLine {
            span_queue: SpanQueue::new(),
            span_line_epoch: epoch,
            parents,
        };

        self.span_lines.push(span_line);
        Some(epoch)
    }

    pub(crate) fn unregister_and_collect(
        &mut self,
        span_line_epoch: usize,
    ) -> Option<(RawSpans, Option<ParentSpans>)> {
        debug_assert_eq!(
            self.current_span_line()
                .map(|span_line| span_line.span_line_epoch),
            Some(span_line_epoch)
        );

        let span_line = self.span_lines.pop()?;
        (span_line.span_line_epoch == span_line_epoch)
            .then(move || (span_line.span_queue.take_queue(), span_line.parents))
    }

    #[inline]
    pub fn add_properties<I, F>(&mut self, local_span_handle: &LocalSpanHandle, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        debug_assert!(self.current_span_line().is_some());

        if let Some(span_line) = self.current_span_line() {
            debug_assert_eq!(span_line.span_line_epoch, local_span_handle.span_line_epoch);

            if span_line.span_line_epoch == local_span_handle.span_line_epoch {
                span_line
                    .span_queue
                    .add_properties(&local_span_handle.span_handle, properties());
            }
        }
    }
}

impl SpanLine {
    #[inline]
    pub fn current_parents(&self) -> Option<ParentSpans> {
        self.parents.as_ref().map(|parents| {
            let mut parents_spans = alloc_parent_spans();
            parents_spans.extend(parents.iter().map(|parent| ParentSpan {
                span_id: self.span_queue.next_parent_id.unwrap_or(parent.span_id),
                collect_id: parent.collect_id,
            }));
            parents_spans
        })
    }
}
