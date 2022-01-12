// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use minstant::Anchor;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use retain_mut::RetainMut;

use crate::collector::SpanRecord;
use crate::local::raw_span::RawSpan;
use crate::local::span_id::SpanId;
use crate::local::LocalSpans;
use crate::util::spsc::{self, Receiver, Sender};
use crate::util::ParentSpans;

const COLLECT_LOOP_INTERVAL: Duration = Duration::from_millis(10);

static NEXT_COLLECT_ID: AtomicU32 = AtomicU32::new(0);
static GLOBAL_COLLECTOR: Lazy<Mutex<GlobalCollector>> =
    Lazy::new(|| Mutex::new(GlobalCollector::start()));

thread_local! {
    static COMMAND_SENDER: Sender<CollectCommand> = {
        let (tx, rx) = spsc::unbounded();
        GLOBAL_COLLECTOR.lock().register_receiver(rx);
        tx
    };
}

pub(crate) fn start_collect() -> u32 {
    let collect_id = NEXT_COLLECT_ID.fetch_add(1, Ordering::AcqRel);
    send_command(CollectCommand::StartCollect { collect_id });
    collect_id
}

pub(crate) fn drop_collect(collect_id: u32) {
    send_command(CollectCommand::DropCollect { collect_id });
}

pub(crate) fn commit_collect(
    collect_id: u32,
) -> futures::channel::oneshot::Receiver<Vec<SpanRecord>> {
    let (tx, rx) = futures::channel::oneshot::channel();
    send_command(CollectCommand::CommitCollect { collect_id, tx });
    rx
}

pub(crate) fn submit_spans(spans: SpanSet, parents: ParentSpans) {
    send_command(CollectCommand::SubmitSpans { spans, parents });
}

fn send_command(cmd: CollectCommand) {
    COMMAND_SENDER.with(|sender| sender.send(cmd));
}

#[derive(Debug)]
pub(crate) enum SpanSet {
    Span(RawSpan),
    LocalSpans(LocalSpans),
    SharedLocalSpans(Arc<LocalSpans>),
}

enum SpanCollection {
    Owned {
        spans: SpanSet,
        parent_id: SpanId,
    },
    Shared {
        spans: Arc<SpanSet>,
        parent_id: SpanId,
    },
}

#[derive(Debug)]
enum CollectCommand {
    StartCollect {
        collect_id: u32,
    },
    DropCollect {
        collect_id: u32,
    },
    CommitCollect {
        collect_id: u32,
        tx: futures::channel::oneshot::Sender<Vec<SpanRecord>>,
    },
    SubmitSpans {
        spans: SpanSet,
        parents: ParentSpans,
    },
}

pub(crate) struct GlobalCollector {
    collection: HashMap<u32, Vec<SpanCollection>>,
    rxs: Vec<Receiver<CollectCommand>>,
    committing: Vec<(u32, futures::channel::oneshot::Sender<Vec<SpanRecord>>)>,
}

impl GlobalCollector {
    fn start() -> Self {
        std::thread::spawn(move || loop {
            let begin_instant = std::time::Instant::now();
            GLOBAL_COLLECTOR.lock().handle_commands();
            std::thread::sleep(COLLECT_LOOP_INTERVAL.saturating_sub(begin_instant.elapsed()));
        });

        GlobalCollector {
            collection: HashMap::new(),
            rxs: Vec::new(),
            committing: Vec::new(),
        }
    }

    fn register_receiver(&mut self, rx: Receiver<CollectCommand>) {
        self.rxs.push(rx);
    }

    fn handle_commands(&mut self) {
        let mut cmds = Vec::with_capacity(128);

        RetainMut::retain_mut(&mut self.rxs, |rx| loop {
            match rx.try_recv() {
                Ok(Some(cmd)) => cmds.push(cmd),
                Ok(None) => {
                    return true;
                }
                Err(_) => {
                    return false;
                }
            }
        });

        cmds.sort_by(|a, b| {
            let to_sort_key = |cmd: &CollectCommand| match *cmd {
                CollectCommand::StartCollect { .. } => 0,
                CollectCommand::DropCollect { .. } => 1,
                CollectCommand::CommitCollect { .. } => 2,
                CollectCommand::SubmitSpans { .. } => 3,
            };

            to_sort_key(a).cmp(&to_sort_key(b))
        });

        let old_committing = std::mem::take(&mut self.committing);

        for cmd in cmds {
            match cmd {
                CollectCommand::StartCollect { collect_id } => {
                    self.collection.insert(collect_id, Vec::new());
                }
                CollectCommand::DropCollect { collect_id } => {
                    self.collection.remove(&collect_id);
                }
                CollectCommand::CommitCollect { collect_id, tx } => {
                    self.committing.push((collect_id, tx));
                }
                CollectCommand::SubmitSpans { spans, parents } => {
                    debug_assert!(!parents.is_empty());

                    if parents.len() == 1 {
                        let parent_span = parents[0];
                        if let Some(buf) = self.collection.get_mut(&parent_span.collect_id) {
                            buf.push(SpanCollection::Owned {
                                spans,
                                parent_id: parent_span.parent_id,
                            });
                        }
                    } else {
                        let spans = Arc::new(spans);
                        for parent_span in parents.iter() {
                            if let Some(buf) = self.collection.get_mut(&parent_span.collect_id) {
                                buf.push(SpanCollection::Shared {
                                    spans: spans.clone(),
                                    parent_id: parent_span.parent_id,
                                });
                            }
                        }
                    }
                }
            }
        }

        for (collect_id, tx) in old_committing {
            let records = self
                .collection
                .remove(&collect_id)
                .map(merge_collection)
                .unwrap_or_else(Vec::new);
            tx.send(records).ok();
        }
    }
}

fn merge_collection(span_collections: Vec<SpanCollection>) -> Vec<SpanRecord> {
    let anchor = Anchor::new();

    let capacity = span_collections
        .iter()
        .map(|sc| match sc.spans() {
            SpanSet::LocalSpans(local_spans) => local_spans.spans.len(),
            SpanSet::SharedLocalSpans(local_spans) => local_spans.spans.len(),
            SpanSet::Span(_) => 1,
        })
        .sum();

    let mut records = Vec::with_capacity(capacity);

    for span_collection in span_collections {
        match span_collection {
            SpanCollection::Owned { spans, parent_id } => match spans {
                SpanSet::Span(raw_span) => amend_span(&raw_span, parent_id, &mut records, &anchor),
                SpanSet::LocalSpans(local_spans) => {
                    amend_local_span(&local_spans, parent_id, &mut records, &anchor)
                }
                SpanSet::SharedLocalSpans(local_spans) => {
                    amend_local_span(&*local_spans, parent_id, &mut records, &anchor)
                }
            },
            SpanCollection::Shared { spans, parent_id } => match &*spans {
                SpanSet::Span(raw_span) => amend_span(&raw_span, parent_id, &mut records, &anchor),
                SpanSet::LocalSpans(local_spans) => {
                    amend_local_span(&local_spans, parent_id, &mut records, &anchor)
                }
                SpanSet::SharedLocalSpans(local_spans) => {
                    amend_local_span(&*local_spans, parent_id, &mut records, &anchor)
                }
            },
        }
    }

    records
}

fn amend_local_span(
    local_spans: &LocalSpans,
    parent_id: SpanId,
    spans: &mut Vec<SpanRecord>,
    anchor: &Anchor,
) {
    for span in local_spans.spans.iter() {
        let begin_unix_time_ns = span.begin_instant.as_unix_nanos(anchor);
        let end_unix_time_ns = if span.end_instant == span.begin_instant {
            local_spans.end_time.as_unix_nanos(anchor)
        } else {
            span.end_instant.as_unix_nanos(anchor)
        };
        let parent_id = if span.parent_id.0 == 0 {
            parent_id.0
        } else {
            span.parent_id.0
        };
        spans.push(SpanRecord {
            id: span.id.0,
            parent_id,
            begin_unix_time_ns,
            duration_ns: end_unix_time_ns - begin_unix_time_ns,
            event: span.event,
            properties: span.properties.clone(),
        });
    }
}

fn amend_span(raw_span: &RawSpan, parent_id: SpanId, spans: &mut Vec<SpanRecord>, anchor: &Anchor) {
    let begin_unix_time_ns = raw_span.begin_instant.as_unix_nanos(anchor);
    let end_unix_time_ns = raw_span.end_instant.as_unix_nanos(anchor);
    spans.push(SpanRecord {
        id: raw_span.id.0,
        parent_id: parent_id.0,
        begin_unix_time_ns,
        duration_ns: end_unix_time_ns - begin_unix_time_ns,
        event: raw_span.event,
        properties: raw_span.properties.clone(),
    });
}

impl SpanCollection {
    fn spans(&self) -> &SpanSet {
        match self {
            SpanCollection::Owned { spans, .. } => spans,
            SpanCollection::Shared { spans, .. } => spans,
        }
    }
}
