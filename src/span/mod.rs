// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

pub mod cycle;
pub mod span_id;
pub mod span_queue;

use crate::span::cycle::{Anchor, Cycle, DefaultClock};
use crate::span::span_id::SpanId;

#[derive(Clone, Debug, Default)]
pub struct Span {
    pub id: u64,
    pub parent_id: u64,
    pub begin_unix_time_us: u64,
    pub duration_ns: u64,
    pub event: &'static str,
    pub properties: Vec<(&'static str, String)>,
}

#[derive(Clone, Debug)]
pub struct RawSpan {
    pub id: SpanId,
    pub parent_id: SpanId,
    pub begin_cycle: Cycle,
    pub event: &'static str,
    pub properties: Vec<(&'static str, String)>,

    // post processing will write this
    pub end_cycle: Cycle,

    // for local queue implementation
    pub(crate) _descendant_count: usize,

    // a tag
    pub(crate) _is_spawn_span: bool,
}

impl RawSpan {
    #[inline]
    pub(crate) fn begin_with(
        id: SpanId,
        parent_id: SpanId,
        begin_cycles: Cycle,
        event: &'static str,
    ) -> Self {
        RawSpan {
            id,
            parent_id,
            begin_cycle: begin_cycles,
            event,
            properties: vec![],
            end_cycle: Cycle::default(),
            _descendant_count: 0,
            _is_spawn_span: false,
        }
    }

    #[inline]
    pub(crate) fn end_with(&mut self, end_cycles: Cycle, descendant_count: usize) {
        self.end_cycle = end_cycles;
        self._descendant_count = descendant_count;
    }

    #[inline]
    pub fn build_span(&self, anchor: Anchor) -> Span {
        let begin_unix_time_us = DefaultClock::cycle_to_unix_time_ns(self.begin_cycle, anchor);
        let end_unix_time_us = DefaultClock::cycle_to_unix_time_ns(self.end_cycle, anchor);
        Span {
            id: self.id.0,
            parent_id: self.parent_id.0,
            begin_unix_time_us,
            duration_ns: end_unix_time_us - begin_unix_time_us,
            event: self.event,
            properties: self.properties.clone(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ScopeSpan {
    pub id: SpanId,
    pub parent_id: SpanId,
    pub begin_cycle: Cycle,
    pub end_cycle: Cycle,
    pub event: &'static str,
}

impl ScopeSpan {
    pub fn new(id: SpanId, parent_id: SpanId, begin_cycles: Cycle, event: &'static str) -> Self {
        ScopeSpan {
            id,
            parent_id,
            begin_cycle: begin_cycles,
            end_cycle: Cycle::new(0),
            event,
        }
    }

    #[inline]
    pub fn build_span(&self, anchor: Anchor) -> Span {
        let begin_unix_time_us = DefaultClock::cycle_to_unix_time_ns(self.begin_cycle, anchor);
        let end_unix_time_us = DefaultClock::cycle_to_unix_time_ns(self.end_cycle, anchor);
        Span {
            id: self.id.0,
            parent_id: self.parent_id.0,
            begin_unix_time_us,
            duration_ns: end_unix_time_us - begin_unix_time_us,
            event: self.event,
            properties: vec![],
        }
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.parent_id == SpanId::new(0)
    }
}
