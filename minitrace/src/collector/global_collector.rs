// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use minstant::Anchor;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use retain_mut::RetainMut;

use crate::collector::{CollectArgs, SpanRecord};
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
        let (tx, rx) = spsc::bounded(10240);
        GLOBAL_COLLECTOR.lock().register_receiver(rx);
        tx
    };
}

pub(crate) fn start_collect(collect_args: CollectArgs) -> u32 {
    let collect_id = NEXT_COLLECT_ID.fetch_add(1, Ordering::AcqRel);
    send_command(CollectCommand::StartCollect(StartCollect {
        collect_id,
        collect_args,
    }));
    collect_id
}

pub(crate) fn drop_collect(collect_id: u32) {
    force_send_command(CollectCommand::DropCollect(DropCollect { collect_id }));
}

pub(crate) fn commit_collect(
    collect_id: u32,
    tx: futures::channel::oneshot::Sender<Vec<SpanRecord>>,
) {
    force_send_command(CollectCommand::CommitCollect(CommitCollect {
        collect_id,
        tx,
    }));
}

pub(crate) fn submit_spans(spans: SpanSet, parents: ParentSpans) {
    send_command(CollectCommand::SubmitSpans(SubmitSpans { spans, parents }));
}

fn send_command(cmd: CollectCommand) {
    COMMAND_SENDER.with(|sender| sender.send(cmd).ok());
}

fn force_send_command(cmd: CollectCommand) {
    COMMAND_SENDER.with(|sender| sender.force_send(cmd));
}

#[derive(Debug)]
pub(crate) enum SpanSet {
    Span(RawSpan),
    LocalSpans(LocalSpans),
    SharedLocalSpans(Arc<LocalSpans>),
}

#[derive(Debug)]
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
    StartCollect(StartCollect),
    DropCollect(DropCollect),
    CommitCollect(CommitCollect),
    SubmitSpans(SubmitSpans),
}

#[derive(Debug)]
struct StartCollect {
    collect_id: u32,
    collect_args: CollectArgs,
}

#[derive(Debug)]
struct DropCollect {
    collect_id: u32,
}

#[derive(Debug)]
struct CommitCollect {
    collect_id: u32,
    tx: futures::channel::oneshot::Sender<Vec<SpanRecord>>,
}

#[derive(Debug)]
struct SubmitSpans {
    spans: SpanSet,
    parents: ParentSpans,
}

pub(crate) struct GlobalCollector {
    active_collector: HashMap<u32, (Vec<SpanCollection>, usize, CollectArgs)>,
    rxs: Vec<Receiver<CollectCommand>>,

    // Vectors to be reused by collection loops. They must be empty outside of the `handle_commands` loop.
    start_collects: Vec<StartCollect>,
    drop_collects: Vec<DropCollect>,
    commit_collects: Vec<CommitCollect>,
    submit_spans: Vec<SubmitSpans>,
}

impl GlobalCollector {
    fn start() -> Self {
        std::thread::spawn(move || loop {
            let begin_instant = std::time::Instant::now();
            GLOBAL_COLLECTOR.lock().handle_commands();
            std::thread::sleep(COLLECT_LOOP_INTERVAL.saturating_sub(begin_instant.elapsed()));
        });

        GlobalCollector {
            active_collector: HashMap::new(),
            rxs: Vec::new(),

            start_collects: Vec::new(),
            drop_collects: Vec::new(),
            commit_collects: Vec::new(),
            submit_spans: Vec::new(),
        }
    }

    fn register_receiver(&mut self, rx: Receiver<CollectCommand>) {
        self.rxs.push(rx);
    }

    fn handle_commands(&mut self) {
        debug_assert!(self.start_collects.is_empty());
        debug_assert!(self.drop_collects.is_empty());
        debug_assert!(self.commit_collects.is_empty());
        debug_assert!(self.submit_spans.is_empty());

        RetainMut::retain_mut(&mut self.rxs, |rx| loop {
            match rx.try_recv() {
                Ok(Some(CollectCommand::StartCollect(cmd))) => self.start_collects.push(cmd),
                Ok(Some(CollectCommand::DropCollect(cmd))) => self.drop_collects.push(cmd),
                Ok(Some(CollectCommand::CommitCollect(cmd))) => self.commit_collects.push(cmd),
                Ok(Some(CollectCommand::SubmitSpans(cmd))) => self.submit_spans.push(cmd),
                Ok(None) => {
                    return true;
                }
                Err(_) => {
                    // Channel disconnected. It must be because the sender thread has stopped.
                    return false;
                }
            }
        });

        for StartCollect {
            collect_id,
            collect_args,
        } in self.start_collects.drain(..)
        {
            self.active_collector
                .insert(collect_id, (Vec::new(), 0, collect_args));
        }

        for DropCollect { collect_id } in self.drop_collects.drain(..) {
            self.active_collector.remove(&collect_id);
        }

        for SubmitSpans { spans, parents } in self.submit_spans.drain(..) {
            debug_assert!(!parents.is_empty());

            if parents.len() == 1 {
                let parent_span = parents[0];
                if let Some((buf, span_count, collect_args)) =
                    self.active_collector.get_mut(&parent_span.collect_id)
                {
                    if *span_count < collect_args.max_span_count.unwrap_or(usize::MAX)
                        || parent_span.span_id == SpanId::new(0)
                    {
                        *span_count += spans.len();
                        buf.push(SpanCollection::Owned {
                            spans,
                            parent_id: parent_span.span_id,
                        });
                    }
                }
            } else {
                let spans = Arc::new(spans);
                for parent_span in parents.iter() {
                    if let Some((buf, span_count, collect_args)) =
                        self.active_collector.get_mut(&parent_span.collect_id)
                    {
                        if *span_count < collect_args.max_span_count.unwrap_or(usize::MAX)
                            || parent_span.span_id == SpanId::new(0)
                        {
                            *span_count += spans.len();
                            buf.push(SpanCollection::Shared {
                                spans: spans.clone(),
                                parent_id: parent_span.span_id,
                            });
                        }
                    }
                }
            }
        }

        for CommitCollect { collect_id, tx } in self.commit_collects.drain(..) {
            let records = self
                .active_collector
                .remove(&collect_id)
                .map(|(span_collections, span_count, _)| {
                    merge_collection(span_collections, span_count)
                })
                .unwrap_or_else(Vec::new);
            tx.send(records).ok();
        }
    }
}

fn merge_collection(span_collections: Vec<SpanCollection>, span_count: usize) -> Vec<SpanRecord> {
    let anchor = Anchor::new();

    let mut records = Vec::with_capacity(span_count);

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
                SpanSet::Span(raw_span) => amend_span(raw_span, parent_id, &mut records, &anchor),
                SpanSet::LocalSpans(local_spans) => {
                    amend_local_span(local_spans, parent_id, &mut records, &anchor)
                }
                SpanSet::SharedLocalSpans(local_spans) => {
                    amend_local_span(local_spans, parent_id, &mut records, &anchor)
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
            duration_ns: end_unix_time_ns.saturating_sub(begin_unix_time_ns),
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
        duration_ns: end_unix_time_ns.saturating_sub(begin_unix_time_ns),
        event: raw_span.event,
        properties: raw_span.properties.clone(),
    });
}

impl SpanSet {
    fn len(&self) -> usize {
        match self {
            SpanSet::LocalSpans(local_spans) => local_spans.spans.len(),
            SpanSet::SharedLocalSpans(local_spans) => local_spans.spans.len(),
            SpanSet::Span(_) => 1,
        }
    }
}
