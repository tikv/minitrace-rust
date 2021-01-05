// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;

use crate::local::observer::Observer;

use crate::span::span_queue::{SpanHandle, SpanQueue};
use crate::span::RawSpan;

thread_local! {
    pub(super) static SPAN_LINE: RefCell<SpanLine> = RefCell::new(SpanLine::with_capacity(1024));
}

pub struct SpanLine {
    span_queue: SpanQueue,

    observer_existing: bool,
    current_observer_epoch: usize,
}

impl SpanLine {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            span_queue: SpanQueue::with_capacity(capacity),
            observer_existing: false,
            current_observer_epoch: 0,
        }
    }

    #[inline]
    pub fn start_span(&mut self, event: &'static str) -> Option<SpanHandle> {
        if !self.observer_existing {
            return None;
        }

        Some(
            self.span_queue
                .start_span(event, self.current_observer_epoch),
        )
    }

    #[inline]
    pub fn finish_span(&mut self, span_handle: SpanHandle) {
        if self.is_valid(&span_handle) {
            self.span_queue.finish_span(span_handle);
        }
    }

    #[inline]
    pub fn register_observer(&mut self) -> Option<Observer> {
        // Only allow one observer per thread
        if self.observer_existing {
            return None;
        }

        self.observer_existing = true;
        self.current_observer_epoch = self.current_observer_epoch.wrapping_add(1);

        Some(Observer {
            observer_epoch: self.current_observer_epoch,
        })
    }

    pub fn unregister_and_collect(&mut self, observer: Observer) -> Vec<RawSpan> {
        debug_assert!(self.observer_existing);
        debug_assert_eq!(observer.observer_epoch, self.current_observer_epoch);

        self.observer_existing = false;
        self.span_queue.take_queue()
    }

    #[inline]
    pub fn add_properties<I: IntoIterator<Item = (&'static str, String)>, F: FnOnce() -> I>(
        &mut self,
        span_handle: &SpanHandle,
        properties: F,
    ) {
        if self.is_valid(span_handle) {
            self.span_queue.add_properties(span_handle, properties);
        }
    }

    #[inline]
    pub fn add_property<F: FnOnce() -> (&'static str, String)>(
        &mut self,
        span_handle: &SpanHandle,
        property: F,
    ) {
        if self.is_valid(span_handle) {
            self.span_queue.add_property(span_handle, property);
        }
    }
}

impl SpanLine {
    #[inline]
    fn is_valid(&self, span_handle: &SpanHandle) -> bool {
        self.observer_existing && span_handle.observer_epoch == self.current_observer_epoch
    }
}
