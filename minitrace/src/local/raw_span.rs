// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use minstant::Instant;

use crate::collector::SpanId;

#[derive(Clone, Debug)]
pub struct RawSpan {
    pub id: SpanId,
    pub parent_id: SpanId,
    pub begin_instant: Instant,
    pub name: &'static str,
    pub properties: Vec<(String, String)>,
    pub is_event: bool,

    // Will write this field at post processing
    pub end_instant: Instant,
}

impl RawSpan {
    #[inline]
    pub(crate) fn begin_with(
        id: SpanId,
        parent_id: SpanId,
        begin_instant: Instant,
        name: &'static str,
        is_event: bool,
    ) -> Self {
        RawSpan {
            id,
            parent_id,
            begin_instant,
            name,
            properties: vec![],
            is_event,
            end_instant: begin_instant,
        }
    }

    #[inline]
    pub(crate) fn end_with(&mut self, end_instant: Instant) {
        self.end_instant = end_instant;
    }
}
