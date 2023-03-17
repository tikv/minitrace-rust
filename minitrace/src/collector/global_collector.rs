// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::command::{
    CollectCommand, CommitCollect, DropCollect, StartCollect, SubmitSpans,
};
use crate::collector::{CollectArgs, SpanRecord, SpanSet};
use crate::local::raw_span::RawSpan;
use crate::local::span_id::SpanId;
use crate::local::LocalSpans;
use crate::util::spsc::{self, Receiver, Sender};
use crate::util::CollectToken;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use minstant::Anchor;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

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

fn send_command(cmd: CollectCommand) {
    COMMAND_SENDER.with(|sender| sender.send(cmd).ok());
}

fn force_send_command(cmd: CollectCommand) {
    COMMAND_SENDER.with(|sender| sender.force_send(cmd));
}

#[derive(Default, Clone)]
pub(crate) struct GlobalCollect;

#[cfg_attr(test, mockall::automock)]
impl GlobalCollect {
    pub fn start_collect(&self, collect_args: CollectArgs) -> u32 {
        let collect_id = NEXT_COLLECT_ID.fetch_add(1, Ordering::Relaxed);
        send_command(CollectCommand::StartCollect(StartCollect {
            collect_id,
            collect_args,
        }));
        collect_id
    }

    pub async fn commit_collect(&self, collect_id: u32) -> Vec<SpanRecord> {
        let (tx, rx) = futures::channel::oneshot::channel();
        force_send_command(CollectCommand::CommitCollect(CommitCollect {
            collect_id,
            tx,
        }));
        rx.await.unwrap_or_else(|_| Vec::new())
    }

    pub fn drop_collect(&self, collect_id: u32) {
        force_send_command(CollectCommand::DropCollect(DropCollect { collect_id }));
    }

    // Note that: relationships are not built completely for now so a further job is needed.
    //
    // Every `SpanSet` has its own root spans whose `raw_span.parent_id`s are equal to `SpanId::default()`.
    //
    // Every root span can have multiple parents where mainly comes from `Span::enter_with_parents`.
    // Those parents are recorded into `CollectToken` which has several `CollectTokenItem`s. Look into
    // a `CollectTokenItem`, `parent_id_of_roots` can be found.
    //
    // For example, we have a `SpanSet::LocalSpans` and a `CollectToken` as follow:
    //
    //     SpanSet::LocalSpans::spans                      CollectToken::parent_id_of_roots
    //     +------+-----------+-----+                      +------------+--------------------+
    //     |  id  | parent_id | ... |                      | collect_id | parent_id_of_roots |
    //     +------+-----------+-----+                      +------------+--------------------+
    //     |  43  |    545    | ... |                      |    1212    |          7         |
    //     |  15  |  default  | ... | <- root span         |    874     |         321        |
    //     | 545  |    15     | ... |                      |    915     |         413        |
    //     |  70  |  default  | ... | <- root span         +------------+--------------------+
    //     +------+-----------+-----+
    //
    // There is a many-to-many mapping. Span#15 has parents Span#7, Span#321 and Span#413, so does Span#70.
    //
    // So the expected further job mentioned above is:
    // * Copy `SpanSet` to the same number of copies as `CollectTokenItem`s, one `SpanSet` to one
    //   `CollectTokenItem`
    // * Amend `raw_span.parent_id` of root spans in `SpanSet` to `parent_id_of_roots` of `CollectTokenItem`
    pub fn submit_spans(&self, spans: SpanSet, collect_token: CollectToken) {
        send_command(CollectCommand::SubmitSpans(SubmitSpans {
            spans,
            collect_token,
        }));
    }
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

pub(crate) struct GlobalCollector {
    active_collectors: HashMap<u32, (Vec<SpanCollection>, usize, CollectArgs)>,
    rxs: Vec<Receiver<CollectCommand>>,

    // Vectors to be reused by collection loops. They must be empty outside of the `handle_commands` loop.
    start_collects: Vec<StartCollect>,
    drop_collects: Vec<DropCollect>,
    commit_collects: Vec<CommitCollect>,
    submit_spans: Vec<SubmitSpans>,
}

impl GlobalCollector {
    fn start() -> Self {
        std::thread::Builder::new()
            .name("minitrace".to_string())
            .spawn(move || loop {
                let begin_instant = std::time::Instant::now();
                GLOBAL_COLLECTOR.lock().handle_commands();
                std::thread::sleep(COLLECT_LOOP_INTERVAL.saturating_sub(begin_instant.elapsed()));
            })
            .unwrap();

        GlobalCollector {
            active_collectors: HashMap::new(),
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

        let start_collects = &mut self.start_collects;
        let drop_collects = &mut self.drop_collects;
        let commit_collects = &mut self.commit_collects;
        let submit_spans = &mut self.submit_spans;

        self.rxs.retain_mut(|rx| loop {
            match rx.try_recv() {
                Ok(Some(CollectCommand::StartCollect(cmd))) => start_collects.push(cmd),
                Ok(Some(CollectCommand::DropCollect(cmd))) => drop_collects.push(cmd),
                Ok(Some(CollectCommand::CommitCollect(cmd))) => commit_collects.push(cmd),
                Ok(Some(CollectCommand::SubmitSpans(cmd))) => submit_spans.push(cmd),
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
            self.active_collectors
                .insert(collect_id, (Vec::new(), 0, collect_args));
        }

        for DropCollect { collect_id } in self.drop_collects.drain(..) {
            self.active_collectors.remove(&collect_id);
        }

        for SubmitSpans {
            spans,
            collect_token,
        } in self.submit_spans.drain(..)
        {
            debug_assert!(!collect_token.is_empty());

            if collect_token.len() == 1 {
                let item = collect_token[0];
                if let Some((buf, span_count, collect_args)) =
                    self.active_collectors.get_mut(&item.collect_id)
                {
                    // The root span, i.e. the span whose parent id is `SpanId::default`, is intended to be kept.
                    if *span_count < collect_args.max_span_count.unwrap_or(usize::MAX)
                        || item.parent_id_of_roots == SpanId::default()
                    {
                        *span_count += spans.len();
                        buf.push(SpanCollection::Owned {
                            spans,
                            parent_id: item.parent_id_of_roots,
                        });
                    }
                }
            } else {
                let spans = Arc::new(spans);
                for item in collect_token.iter() {
                    if let Some((buf, span_count, collect_args)) =
                        self.active_collectors.get_mut(&item.collect_id)
                    {
                        // Multiple items in a collect token are built from `Span::enter_from_parents`,
                        // so relative span cannot be a root span.
                        if *span_count < collect_args.max_span_count.unwrap_or(usize::MAX) {
                            *span_count += spans.len();
                            buf.push(SpanCollection::Shared {
                                spans: spans.clone(),
                                parent_id: item.parent_id_of_roots,
                            });
                        }
                    }
                }
            }
        }

        for CommitCollect { collect_id, tx } in self.commit_collects.drain(..) {
            let records = self
                .active_collectors
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
                    amend_local_span(&local_spans, parent_id, &mut records, &anchor)
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
