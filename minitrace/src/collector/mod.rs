// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

pub(crate) mod global_collector;

use crate::local::span_id::SpanId;

#[derive(Clone, Debug, Default)]
pub struct SpanRecord {
    pub id: u32,
    pub parent_id: u32,
    pub begin_unix_time_ns: u64,
    pub duration_ns: u64,
    pub event: &'static str,
    pub properties: Vec<(&'static str, String)>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ParentSpan {
    pub(crate) parent_id: SpanId,
    pub(crate) collect_id: u32,
}

pub struct Collector {
    pub(crate) collect_id: u32,
}

impl Collector {
    pub fn new() -> Self {
        Collector {
            collect_id: global_collector::start_collect(),
        }
    }

    pub async fn collect(self) -> Vec<SpanRecord> {
        global_collector::commit_collect(self.collect_id)
            .await
            .unwrap()
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        global_collector::drop_collect(self.collect_id);
    }
}
