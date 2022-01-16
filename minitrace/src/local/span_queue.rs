// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::raw_span::RawSpan;
use crate::local::span_id::{DefaultIdGenerator, SpanId};
use crate::util::{alloc_raw_spans, RawSpans};

use minstant::Instant;

const DEFAULT_SPAN_QUEUE_SIZE: usize = 4096;

#[derive(Debug)]
pub(crate) struct SpanQueue {
    span_queue: RawSpans,
    capacity: usize,
    pub(crate) next_parent_id: Option<SpanId>,
}

pub(crate) struct SpanHandle {
    pub(crate) index: usize,
}

impl SpanQueue {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_SPAN_QUEUE_SIZE)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        let span_queue = alloc_raw_spans();
        Self {
            span_queue,
            capacity,
            next_parent_id: None,
        }
    }

    #[inline]
    pub fn start_span(&mut self, event: &'static str) -> Option<SpanHandle> {
        if self.span_queue.len() >= self.capacity {
            return None;
        }

        let span = RawSpan::begin_with(
            DefaultIdGenerator::next_id(),
            self.next_parent_id.unwrap_or(SpanId(0)),
            Instant::now(),
            event,
        );
        self.next_parent_id = Some(span.id);

        let index = self.span_queue.len();
        self.span_queue.push(span);

        Some(SpanHandle { index })
    }

    #[inline]
    pub fn finish_span(&mut self, span_handle: SpanHandle) {
        debug_assert!(span_handle.index < self.span_queue.len());
        debug_assert_eq!(
            self.next_parent_id,
            Some(self.span_queue[span_handle.index].id)
        );

        let span = &mut self.span_queue[span_handle.index];
        span.end_with(Instant::now());

        self.next_parent_id = Some(span.parent_id).filter(|id| id.0 != 0);
    }

    #[inline]
    pub fn add_properties<I: IntoIterator<Item = (&'static str, String)>>(
        &mut self,
        span_handle: &SpanHandle,
        properties: I,
    ) {
        debug_assert!(span_handle.index < self.span_queue.len());

        let span = &mut self.span_queue[span_handle.index];
        span.properties.extend(properties);
    }

    #[inline]
    pub fn take_queue(self) -> RawSpans {
        self.span_queue
    }
}
