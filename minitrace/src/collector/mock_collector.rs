// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::{Collect, CollectArgs, ParentSpan, SpanRecord, SpanSet};
use crate::local::span_id::SpanId;
use crate::util::ParentSpans;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MockCollect {
    pub inner: Arc<MockCollectInner>,
}

pub struct MockCollectInner {
    id: AtomicU32,
    pub active_collects: Mutex<HashMap<u32, Vec<(SpanSet, SpanId)>>>,
    pub finished_collects: Mutex<HashMap<u32, Vec<(SpanSet, SpanId)>>>,
}

impl Collect for MockCollect {
    fn start_collect(&self, _: CollectArgs) -> u32 {
        let id = self.inner.id.fetch_add(1, Ordering::SeqCst);
        let collects = &mut *self.inner.active_collects.lock().unwrap();
        collects.insert(id, Vec::new());
        id
    }

    fn commit_collect(
        &self,
        collect_id: u32,
        _: futures::channel::oneshot::Sender<Vec<SpanRecord>>,
    ) {
        let spans = {
            let collects = &mut *self.inner.active_collects.lock().unwrap();
            collects.remove(&collect_id).unwrap()
        };

        let collects = &mut *self.inner.finished_collects.lock().unwrap();
        collects.insert(collect_id, spans);
    }

    fn drop_collect(&self, collect_id: u32) {
        let collects = &mut *self.inner.active_collects.lock().unwrap();
        collects.remove(&collect_id).unwrap();
    }

    fn submit_spans(&self, spans: SpanSet, parents: ParentSpans) {
        let collects = &mut *self.inner.active_collects.lock().unwrap();

        match spans {
            s @ SpanSet::Span(_) | s @ SpanSet::LocalSpans(_) => {
                assert_eq!(parents.len(), 1);
                collects
                    .get_mut(&parents[0].collect_id)
                    .unwrap()
                    .push((s, parents[0].span_id));
            }
            SpanSet::SharedLocalSpans(spans) => {
                for ParentSpan {
                    span_id,
                    collect_id,
                } in parents.iter()
                {
                    let v = collects.get_mut(&collect_id).unwrap();
                    v.push((SpanSet::SharedLocalSpans(spans.clone()), *span_id));
                }
            }
        }
    }
}
