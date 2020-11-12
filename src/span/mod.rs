// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

pub mod cycle;
pub mod span_id;
pub mod span_queue;

use crate::span::cycle::Cycle;
use crate::span::span_id::SpanId;

#[derive(Clone, Debug)]
pub struct Span {
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

impl Span {
    #[inline]
    pub(crate) fn begin_with(
        id: SpanId,
        parent_id: SpanId,
        begin_cycles: Cycle,
        event: &'static str,
    ) -> Self {
        Span {
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
    pub fn is_root(&self) -> bool {
        self.parent_id == SpanId::new(0)
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

    pub fn into_span(self) -> Span {
        Span {
            id: self.id,
            parent_id: self.parent_id,
            begin_cycle: self.begin_cycle,
            event: self.event,
            properties: vec![],
            end_cycle: self.end_cycle,
            _descendant_count: 0,
            _is_spawn_span: false,
        }
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.parent_id == SpanId::new(0)
    }
}
