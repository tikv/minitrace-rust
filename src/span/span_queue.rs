// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collections::queue::FixedIndexQueue;
use crate::span::cycle::{Cycle, DefaultClock};
use crate::span::span_id::{DefaultIdGenerator, SpanId};
use crate::span::{ScopeSpan, Span};
use std::collections::VecDeque;

pub struct SpanQueue {
    span_queue: FixedIndexQueue<Span>,
    next_parent_id: SpanId,
}

impl SpanQueue {
    pub fn new() -> Self {
        Self {
            span_queue: FixedIndexQueue::with_capacity(1024),
            next_parent_id: SpanId::new(0),
        }
    }

    #[inline]
    pub fn start_span(&mut self, event: &'static str) -> SpanHandle {
        let s = self.gen_span(self.next_parent_id, event);
        self.next_parent_id = s.id;
        let index = self.push_span(s);
        SpanHandle { index }
    }

    #[inline]
    pub fn finish_span(&mut self, span_handle: SpanHandle) {
        debug_assert!(self.span_queue.idx_is_valid(span_handle.index));

        let descendant_count = self.count_to_last(span_handle.index);
        let span = &mut self.span_queue[span_handle.index];
        span.end_with(DefaultClock::now(), descendant_count);

        self.next_parent_id = span.parent_id;
    }

    #[inline]
    pub fn add_properties<I: IntoIterator<Item = (&'static str, String)>, F: FnOnce() -> I>(
        &mut self,
        span_handle: &SpanHandle,
        properties: F,
    ) {
        debug_assert!(self.span_queue.idx_is_valid(span_handle.index));

        let span = &mut self.span_queue[span_handle.index];
        span.properties.extend(properties());
    }

    #[inline]
    pub fn add_property<F: FnOnce() -> (&'static str, String)>(
        &mut self,
        span_handle: &SpanHandle,
        property: F,
    ) {
        debug_assert!(self.span_queue.idx_is_valid(span_handle.index));

        let span = &mut self.span_queue[span_handle.index];
        span.properties.push(property());
    }

    #[inline]
    pub fn start_scope_span(
        &mut self,
        placeholder_event: &'static str,
        event: &'static str,
    ) -> ScopeSpan {
        // add a spawn span for indirectly linking to the external span
        let mut s = self.gen_span(self.next_parent_id, placeholder_event);
        let cycle = s.begin_cycle;
        s.end_cycle = cycle;
        s._is_spawn_span = true;
        let es_parent = s.id;
        self.push_span(s);

        self.gen_scope_span(es_parent, event, cycle)
    }

    #[inline]
    pub fn next_index(&self) -> usize {
        self.span_queue.next_index()
    }

    #[inline]
    pub fn remove_before(&mut self, index: usize) {
        self.span_queue.remove_before(index);
    }

    #[inline]
    pub fn clone_queue_from(&self, index: usize) -> VecDeque<Span> {
        self.span_queue.clone_queue_from(index)
    }

    #[inline]
    pub fn take_queue_from(&mut self, index: usize) -> VecDeque<Span> {
        self.span_queue.take_queue_from(index)
    }
}

impl SpanQueue {
    #[inline]
    fn gen_span(&self, parent_id: SpanId, event: &'static str) -> Span {
        Span::begin_with(
            DefaultIdGenerator::next_id(),
            parent_id,
            DefaultClock::now(),
            event,
        )
    }

    #[inline]
    fn gen_scope_span(
        &self,
        parent_id: SpanId,
        event: &'static str,
        begin_cycle: Cycle,
    ) -> ScopeSpan {
        ScopeSpan::new(DefaultIdGenerator::next_id(), parent_id, begin_cycle, event)
    }

    #[inline]
    fn push_span(&mut self, span: Span) -> usize {
        self.span_queue.push_back(span)
    }

    fn count_to_last(&self, index: usize) -> usize {
        let next_index = self.span_queue.next_index();
        next_index.wrapping_sub(index) - 1
    }
}

pub struct SpanHandle {
    pub(self) index: usize,
}
