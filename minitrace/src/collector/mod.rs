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
    pub(crate) span_id: SpanId,
    pub(crate) collect_id: u32,
}

pub struct Collector {
    collect_id: u32,
}

impl Collector {
    pub(crate) fn start_collect(args: CollectArgs) -> (Self, u32) {
        let collect_id = global_collector::start_collect(args);

        (Collector { collect_id }, collect_id)
    }

    pub async fn collect(self) -> Vec<SpanRecord> {
        let (tx, rx) = futures::channel::oneshot::channel();
        global_collector::commit_collect(self.collect_id, tx);

        // Don't drop the collect because the collect is committed.
        std::mem::forget(self);

        rx.await.unwrap_or_else(|_| Vec::new())
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        global_collector::drop_collect(self.collect_id);
    }
}

#[must_use]
#[derive(Default, Debug)]
pub struct CollectArgs {
    pub(crate) max_span_count: Option<usize>,
}

impl CollectArgs {
    pub fn max_span_count(self, max_span_count: Option<usize>) -> Self {
        Self { max_span_count }
    }
}
