// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use minstant::Instant;

use crate::collector::SpanId;
use crate::util::Properties;

#[derive(Debug)]
pub struct RawSpan {
    pub id: SpanId,
    pub parent_id: SpanId,
    pub begin_instant: Instant,
    pub name: &'static str,
    pub properties: Properties,
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
            properties: Properties::default(),
            is_event,
            end_instant: begin_instant,
        }
    }

    #[inline]
    pub(crate) fn end_with(&mut self, end_instant: Instant) {
        self.end_instant = end_instant;
    }
}

impl Clone for RawSpan {
    fn clone(&self) -> Self {
        let mut properties = Properties::default();
        properties.extend(self.properties.iter().cloned());

        RawSpan {
            id: self.id,
            parent_id: self.parent_id,
            begin_instant: self.begin_instant,
            name: self.name,
            properties: properties,
            is_event: self.is_event,
            end_instant: self.end_instant,
        }
    }
}
