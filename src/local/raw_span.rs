// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use minstant::Cycle;

use crate::local::span_id::SpanId;

#[derive(Clone, Debug)]
pub(crate) struct RawSpan {
    pub id: SpanId,
    pub parent_id: SpanId,
    pub begin_cycle: Cycle,
    pub event: &'static str,
    pub properties: Vec<(&'static str, String)>,

    // Will write this field at post processing
    pub end_cycle: Cycle,
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
        }
    }

    #[inline]
    pub(crate) fn end_with(&mut self, end_cycle: Cycle) {
        self.end_cycle = end_cycle;
    }
}
