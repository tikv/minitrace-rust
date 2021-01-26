// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;

use crate::local::local_collector::LocalCollector;
use crate::span::span_queue::{SpanHandle, SpanQueue};
use crate::span::RawSpan;

thread_local! {
    pub(super) static SPAN_LINE: RefCell<SpanLine> = RefCell::new(SpanLine::with_capacity(1024));
}

pub struct SpanLine {
    span_queue: SpanQueue,

    local_collector_existing: bool,
    current_local_collector_epoch: usize,
}

pub struct LocalSpanHandle {
    span_handle: SpanHandle,
    local_collector_epoch: usize,
}

impl SpanLine {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            span_queue: SpanQueue::with_capacity(capacity),
            local_collector_existing: false,
            current_local_collector_epoch: 0,
        }
    }

    #[inline]
    pub fn enter_span(&mut self, event: &'static str) -> Option<LocalSpanHandle> {
        if !self.local_collector_existing {
            return None;
        }

        Some(LocalSpanHandle {
            span_handle: self.span_queue.start_span(event),
            local_collector_epoch: self.current_local_collector_epoch,
        })
    }

    #[inline]
    pub fn exit_span(&mut self, local_span_handle: LocalSpanHandle) {
        if self.is_valid(&local_span_handle) {
            self.span_queue.finish_span(local_span_handle.span_handle);
        }
    }

    #[inline]
    pub fn register_local_collector(&mut self) -> Option<LocalCollector> {
        // Only allow one local collector per thread
        if self.local_collector_existing {
            return None;
        }

        self.local_collector_existing = true;
        self.current_local_collector_epoch = self.current_local_collector_epoch.wrapping_add(1);

        Some(LocalCollector::new(self.current_local_collector_epoch))
    }

    pub fn unregister_and_collect(&mut self, local_collector: LocalCollector) -> Vec<RawSpan> {
        debug_assert!(self.local_collector_existing);
        debug_assert_eq!(
            local_collector.local_collector_epoch,
            self.current_local_collector_epoch
        );

        self.local_collector_existing = false;
        self.span_queue.take_queue()
    }

    pub fn clear(&mut self) {
        self.local_collector_existing = false;
        self.span_queue.clear();
    }

    #[inline]
    pub fn add_properties<I: IntoIterator<Item = (&'static str, String)>, F: FnOnce() -> I>(
        &mut self,
        local_span_handle: &LocalSpanHandle,
        properties: F,
    ) {
        if self.is_valid(local_span_handle) {
            self.span_queue
                .add_properties(&local_span_handle.span_handle, properties);
        }
    }

    #[inline]
    pub fn add_property<F: FnOnce() -> (&'static str, String)>(
        &mut self,
        local_span_handle: &LocalSpanHandle,
        property: F,
    ) {
        if self.is_valid(local_span_handle) {
            self.span_queue
                .add_property(&local_span_handle.span_handle, property);
        }
    }
}

impl SpanLine {
    #[inline]
    fn is_valid(&self, local_span_handle: &LocalSpanHandle) -> bool {
        self.local_collector_existing
            && local_span_handle.local_collector_epoch == self.current_local_collector_epoch
    }
}
