// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minstant::Cycle;

use crate::local::raw_span::RawSpan;
use crate::local::span_id::{DefaultIdGenerator, SpanId};

pub(crate) struct SpanQueue {
    span_queue: Vec<RawSpan>,
    pub(crate) next_parent_id: Option<SpanId>,
}

pub(crate) struct SpanHandle {
    pub(crate) index: usize,
}

impl SpanQueue {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            span_queue: Vec::with_capacity(capacity),
            next_parent_id: None,
        }
    }

    #[inline]
    pub fn start_span(&mut self, event: &'static str) -> SpanHandle {
        let span = RawSpan::begin_with(
            DefaultIdGenerator::next_id(),
            self.next_parent_id.unwrap_or(SpanId(0)),
            Cycle::now(),
            event,
        );
        self.next_parent_id = Some(span.id);

        let index = self.span_queue.len();
        self.span_queue.push(span);

        SpanHandle { index }
    }

    #[inline]
    pub fn finish_span(&mut self, span_handle: SpanHandle) {
        debug_assert!(span_handle.index < self.span_queue.len());
        debug_assert_eq!(
            self.next_parent_id,
            Some(self.span_queue[span_handle.index].id)
        );

        let span = &mut self.span_queue[span_handle.index];
        span.end_with(Cycle::now());

        self.next_parent_id = Some(span.parent_id);
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
    pub fn take_queue(&mut self) -> Vec<RawSpan> {
        self.next_parent_id = None;
        self.span_queue.split_off(0)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.next_parent_id = None;
        self.span_queue.clear();
    }
}
