// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::span::cycle::DefaultClock;
use crate::span::span_id::{DefaultIdGenerator, SpanId};
use crate::span::RawSpan;

pub struct SpanQueue {
    span_queue: Vec<RawSpan>,
    next_parent_id: SpanId,
}

pub struct SpanHandle {
    pub(crate) index: usize,
}

impl SpanQueue {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            span_queue: Vec::with_capacity(capacity),
            next_parent_id: SpanId::new(0),
        }
    }

    #[inline]
    pub fn start_span(&mut self, event: &'static str) -> SpanHandle {
        let span = RawSpan::begin_with(
            DefaultIdGenerator::next_id(),
            self.next_parent_id,
            DefaultClock::now(),
            event,
        );
        self.next_parent_id = span.id;

        let index = self.span_queue.len();
        self.span_queue.push(span);

        SpanHandle { index }
    }

    #[inline]
    pub fn finish_span(&mut self, span_handle: SpanHandle) {
        debug_assert!(span_handle.index < self.span_queue.len());
        debug_assert_eq!(self.next_parent_id, self.span_queue[span_handle.index].id);

        let span = &mut self.span_queue[span_handle.index];
        span.end_with(DefaultClock::now());

        self.next_parent_id = span.parent_id;
    }

    #[inline]
    pub fn add_properties<I: IntoIterator<Item = (String, String)>>(
        &mut self,
        span_handle: &SpanHandle,
        properties: I,
    ) {
        debug_assert!(span_handle.index < self.span_queue.len());

        let span = &mut self.span_queue[span_handle.index];
        span.properties.extend(properties);
    }

    #[inline]
    pub fn add_property(&mut self, span_handle: &SpanHandle, property: (String, String)) {
        debug_assert!(span_handle.index < self.span_queue.len());

        let span = &mut self.span_queue[span_handle.index];
        span.properties.push(property);
    }

    #[inline]
    pub fn take_queue(&mut self) -> Vec<RawSpan> {
        self.next_parent_id = SpanId::new(0);
        self.span_queue.split_off(0)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.next_parent_id = SpanId::new(0);
        self.span_queue.clear();
    }
}
