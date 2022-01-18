// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::local_span_line::{LocalSpanHandle, SpanLine};
use crate::util::{CollectToken, RawSpans};

use std::cell::RefCell;
use std::rc::Rc;

const DEFAULT_SPAN_STACK_SIZE: usize = 4096;
const DEFAULT_SPAN_QUEUE_SIZE: usize = 10240;

thread_local! {
    pub static LOCAL_SPAN_STACK: Rc<RefCell<LocalSpanStack>> = Rc::new(RefCell::new(LocalSpanStack::with_capacity(DEFAULT_SPAN_STACK_SIZE)));
}

#[derive(Debug)]
pub struct LocalSpanStack {
    span_lines: Vec<SpanLine>,
    capacity: usize,
    next_span_line_epoch: usize,
}

impl LocalSpanStack {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            span_lines: Vec::with_capacity(capacity / 8),
            capacity,
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
        span_line.start_span(event)
    }

    #[inline]
    pub fn exit_span(&mut self, local_span_handle: LocalSpanHandle) {
        if let Some(span_line) = self.current_span_line() {
            debug_assert_eq!(
                span_line.span_line_epoch(),
                local_span_handle.span_line_epoch
            );
            span_line.finish_span(local_span_handle);
        }
    }

    /// Register a new span line to the span stack. If succeed, return a span line epoch which can
    /// be used to unregister the span line via [`LocalSpanStack::unregister_and_collect`]. If
    /// the size of the span stack is greater than the `capacity`, registration will fail
    /// and a `None` will be returned.
    ///
    /// [`LocalSpanStack::unregister_and_collect`](LocalSpanStack::unregister_and_collect)
    #[inline]
    pub fn register_span_line(
        &mut self,
        collect_token: Option<CollectToken>,
    ) -> Option<SpanLineHandle> {
        if self.span_lines.len() >= self.capacity {
            return None;
        }

        let epoch = self.next_span_line_epoch;
        self.next_span_line_epoch = self.next_span_line_epoch.wrapping_add(1);

        let span_line = SpanLine::new(DEFAULT_SPAN_QUEUE_SIZE, epoch, collect_token);
        self.span_lines.push(span_line);
        Some(SpanLineHandle {
            span_line_epoch: epoch,
        })
    }

    pub fn unregister_and_collect(
        &mut self,
        span_line_handle: SpanLineHandle,
    ) -> Option<(RawSpans, Option<CollectToken>)> {
        debug_assert_eq!(
            self.current_span_line().unwrap().span_line_epoch(),
            span_line_handle.span_line_epoch,
        );
        let span_line = self.span_lines.pop()?;
        span_line.collect(span_line_handle.span_line_epoch)
    }

    pub fn current_collect_token(&mut self) -> Option<CollectToken> {
        let span_line = self.current_span_line()?;
        span_line.current_collect_token()
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
                span_line.span_line_epoch(),
                local_span_handle.span_line_epoch
            );
            span_line.add_properties(local_span_handle, properties);
        }
    }
}

#[derive(Debug)]
pub struct SpanLineHandle {
    span_line_epoch: usize,
}
