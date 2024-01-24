// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use minstant::Anchor;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use super::EventRecord;
use super::SpanContext;
use crate::collector::command::CollectCommand;
use crate::collector::command::CommitCollect;
use crate::collector::command::DropCollect;
use crate::collector::command::StartCollect;
use crate::collector::command::SubmitSpans;
use crate::collector::Config;
use crate::collector::SpanId;
use crate::collector::SpanRecord;
use crate::collector::SpanSet;
use crate::collector::TraceId;
use crate::local::local_collector::LocalSpansInner;
use crate::local::raw_span::RawSpan;
use crate::util::spsc::Receiver;
use crate::util::spsc::Sender;
use crate::util::spsc::{self};
use crate::util::CollectToken;

const COLLECT_LOOP_INTERVAL: Duration = Duration::from_millis(50);

static NEXT_COLLECT_ID: AtomicUsize = AtomicUsize::new(0);
static GLOBAL_COLLECTOR: Lazy<Mutex<GlobalCollector>> =
    Lazy::new(|| Mutex::new(GlobalCollector::start()));
static SPSC_RXS: Lazy<Mutex<Vec<Receiver<CollectCommand>>>> = Lazy::new(|| Mutex::new(Vec::new()));
static REPORTER_READY: AtomicBool = AtomicBool::new(false);

thread_local! {
    static COMMAND_SENDER: UnsafeCell<Sender<CollectCommand>> = {
        let (tx, rx) = spsc::bounded(10240);
        register_receiver(rx);
        UnsafeCell::new(tx)
    };
}

fn register_receiver(rx: Receiver<CollectCommand>) {
    SPSC_RXS.lock().push(rx);
}

fn send_command(cmd: CollectCommand) {
    COMMAND_SENDER
        .try_with(|sender| unsafe { (*sender.get()).send(cmd).ok() })
        .ok();
}

fn force_send_command(cmd: CollectCommand) {
    COMMAND_SENDER
        .try_with(|sender| unsafe { (*sender.get()).force_send(cmd) })
        .ok();
}

/// Sets the reporter and its configuration for the current application.
///
/// # Examples
///
/// ```
/// use minitrace::collector::Config;
/// use minitrace::collector::ConsoleReporter;
///
/// minitrace::set_reporter(ConsoleReporter, Config::default());
/// ```
pub fn set_reporter(reporter: impl Reporter, config: Config) {
    #[cfg(feature = "enable")]
    {
        let mut global_collector = GLOBAL_COLLECTOR.lock();
        global_collector.config = config;
        global_collector.reporter = Some(Box::new(reporter));
        REPORTER_READY.store(true, Ordering::Relaxed);
    }
}

pub(crate) fn reporter_ready() -> bool {
    REPORTER_READY.load(Ordering::Relaxed)
}

/// Flushes all pending span records to the reporter immediately.
pub fn flush() {
    #[cfg(feature = "enable")]
    {
        // Spawns a new thread to ensure the reporter operates outside the tokio runtime to prevent panic.
        std::thread::Builder::new()
            .name("minitrace-flush".to_string())
            .spawn(move || {
                let mut global_collector = GLOBAL_COLLECTOR.lock();
                global_collector.handle_commands(true);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

/// A trait defining the behavior of a reporter. A reporter is responsible for
/// handling span records, typically by sending them to a remote service for
/// further processing and analysis.
pub trait Reporter: Send + 'static {
    /// Reports a batch of spans to a remote service.
    fn report(&mut self, spans: &[SpanRecord]);
}

#[derive(Default, Clone)]
pub(crate) struct GlobalCollect;

#[cfg_attr(test, mockall::automock)]
impl GlobalCollect {
    pub fn start_collect(&self) -> usize {
        let collect_id = NEXT_COLLECT_ID.fetch_add(1, Ordering::Relaxed);
        send_command(CollectCommand::StartCollect(StartCollect { collect_id }));
        collect_id
    }

    pub fn commit_collect(&self, collect_id: usize) {
        force_send_command(CollectCommand::CommitCollect(CommitCollect { collect_id }));
    }

    pub fn drop_collect(&self, collect_id: usize) {
        force_send_command(CollectCommand::DropCollect(DropCollect { collect_id }));
    }

    // Note that: relationships are not built completely for now so a further job is needed.
    //
    // Every `SpanSet` has its own root spans whose `raw_span.parent_id`s are equal to `SpanId::default()`.
    //
    // Every root span can have multiple parents where mainly comes from `Span::enter_with_parents`.
    // Those parents are recorded into `CollectToken` which has several `CollectTokenItem`s. Look into
    // a `CollectTokenItem`, `parent_ids` can be found.
    //
    // For example, we have a `SpanSet::LocalSpansInner` and a `CollectToken` as follow:
    //
    //     SpanSet::LocalSpansInner::spans                  CollectToken::parent_ids
    //     +------+-----------+-----+                      +------------+------------+
    //     |  id  | parent_id | ... |                      | collect_id | parent_ids |
    //     +------+-----------+-----+                      +------------+------------+
    //     |  43  |    545    | ... |                      |    1212    |      7     |
    //     |  15  |  default  | ... | <- root span         |    874     |     321    |
    //     | 545  |    15     | ... |                      |    915     |     413    |
    //     |  70  |  default  | ... | <- root span         +------------+------------+
    //     +------+-----------+-----+
    //
    // There is a many-to-many mapping. Span#15 has parents Span#7, Span#321 and Span#413, so does Span#70.
    //
    // So the expected further job mentioned above is:
    // * Copy `SpanSet` to the same number of copies as `CollectTokenItem`s, one `SpanSet` to one
    //   `CollectTokenItem`
    // * Amend `raw_span.parent_id` of root spans in `SpanSet` to `parent_ids` of `CollectTokenItem`
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
        trace_id: TraceId,
        parent_id: SpanId,
    },
    Shared {
        spans: Arc<SpanSet>,
        trace_id: TraceId,
        parent_id: SpanId,
    },
}

pub(crate) struct GlobalCollector {
    config: Config,
    reporter: Option<Box<dyn Reporter>>,

    active_collectors: HashMap<usize, (Vec<SpanCollection>, usize)>,
    committed_records: Vec<SpanRecord>,
    last_report: std::time::Instant,

    // Vectors to be reused by collection loops. They must be empty outside of the `handle_commands` loop.
    start_collects: Vec<StartCollect>,
    drop_collects: Vec<DropCollect>,
    commit_collects: Vec<CommitCollect>,
    submit_spans: Vec<SubmitSpans>,
    dangling_events: HashMap<SpanId, Vec<EventRecord>>,
}

impl GlobalCollector {
    #[allow(unreachable_code)]
    fn start() -> Self {
        #[cfg(not(feature = "enable"))]
        {
            unreachable!(
                "Global collector should not be invoked because feature \"enable\" is not set."
            )
        }

        std::thread::Builder::new()
            .name("minitrace-global-collector".to_string())
            .spawn(move || {
                loop {
                    let begin_instant = std::time::Instant::now();
                    GLOBAL_COLLECTOR.lock().handle_commands(false);
                    std::thread::sleep(
                        COLLECT_LOOP_INTERVAL.saturating_sub(begin_instant.elapsed()),
                    );
                }
            })
            .unwrap();

        GlobalCollector {
            config: Config::default().max_spans_per_trace(Some(0)),
            reporter: None,

            active_collectors: HashMap::new(),
            committed_records: Vec::new(),
            last_report: std::time::Instant::now(),

            start_collects: Vec::new(),
            drop_collects: Vec::new(),
            commit_collects: Vec::new(),
            submit_spans: Vec::new(),
            dangling_events: HashMap::new(),
        }
    }

    fn handle_commands(&mut self, flush: bool) {
        debug_assert!(self.start_collects.is_empty());
        debug_assert!(self.drop_collects.is_empty());
        debug_assert!(self.commit_collects.is_empty());
        debug_assert!(self.submit_spans.is_empty());
        debug_assert!(self.dangling_events.is_empty());

        let start_collects = &mut self.start_collects;
        let drop_collects = &mut self.drop_collects;
        let commit_collects = &mut self.commit_collects;
        let submit_spans = &mut self.submit_spans;
        let committed_records = &mut self.committed_records;

        {
            SPSC_RXS.lock().retain_mut(|rx| {
                loop {
                    match rx.try_recv() {
                        Ok(Some(CollectCommand::StartCollect(cmd))) => start_collects.push(cmd),
                        Ok(Some(CollectCommand::DropCollect(cmd))) => drop_collects.push(cmd),
                        Ok(Some(CollectCommand::CommitCollect(cmd))) => commit_collects.push(cmd),
                        Ok(Some(CollectCommand::SubmitSpans(cmd))) => submit_spans.push(cmd),
                        Ok(None) => {
                            // Channel is empty.
                            return true;
                        }
                        Err(_) => {
                            // Channel closed. Remove it from the channel list.
                            return false;
                        }
                    }
                }
            });
        }

        // If the reporter is not set, global collectior only clears the channel and then dismiss all messages.
        if self.reporter.is_none() {
            start_collects.clear();
            drop_collects.clear();
            commit_collects.clear();
            submit_spans.clear();
            return;
        }

        for StartCollect { collect_id } in self.start_collects.drain(..) {
            self.active_collectors.insert(collect_id, (Vec::new(), 0));
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
                if let Some((buf, span_count)) = self.active_collectors.get_mut(&item.collect_id) {
                    if *span_count < self.config.max_spans_per_trace.unwrap_or(usize::MAX)
                        || item.is_root
                    {
                        *span_count += spans.len();
                        buf.push(SpanCollection::Owned {
                            spans,
                            trace_id: item.trace_id,
                            parent_id: item.parent_id,
                        });
                    }
                }
            } else {
                let spans = Arc::new(spans);
                for item in collect_token.iter() {
                    if let Some((buf, span_count)) =
                        self.active_collectors.get_mut(&item.collect_id)
                    {
                        // Multiple items in a collect token are built from `Span::enter_from_parents`,
                        // so relative span cannot be a root span.
                        if *span_count < self.config.max_spans_per_trace.unwrap_or(usize::MAX) {
                            *span_count += spans.len();
                            buf.push(SpanCollection::Shared {
                                spans: spans.clone(),
                                trace_id: item.trace_id,
                                parent_id: item.parent_id,
                            });
                        }
                    }
                }
            }
        }

        for CommitCollect { collect_id } in commit_collects.drain(..) {
            if let Some((span_collections, _)) = self.active_collectors.remove(&collect_id) {
                debug_assert!(self.dangling_events.is_empty());
                let dangling_events = &mut self.dangling_events;

                let anchor: Anchor = Anchor::new();
                let committed_len = committed_records.len();

                for span_collection in span_collections {
                    match span_collection {
                        SpanCollection::Owned {
                            spans,
                            trace_id,
                            parent_id,
                        } => match spans {
                            SpanSet::Span(raw_span) => amend_span(
                                &raw_span,
                                trace_id,
                                parent_id,
                                committed_records,
                                dangling_events,
                                &anchor,
                            ),
                            SpanSet::LocalSpansInner(local_spans) => amend_local_span(
                                &local_spans,
                                trace_id,
                                parent_id,
                                committed_records,
                                dangling_events,
                                &anchor,
                            ),
                            SpanSet::SharedLocalSpans(local_spans) => amend_local_span(
                                &local_spans,
                                trace_id,
                                parent_id,
                                committed_records,
                                dangling_events,
                                &anchor,
                            ),
                        },
                        SpanCollection::Shared {
                            spans,
                            trace_id,
                            parent_id,
                        } => match &*spans {
                            SpanSet::Span(raw_span) => amend_span(
                                raw_span,
                                trace_id,
                                parent_id,
                                committed_records,
                                dangling_events,
                                &anchor,
                            ),
                            SpanSet::LocalSpansInner(local_spans) => amend_local_span(
                                local_spans,
                                trace_id,
                                parent_id,
                                committed_records,
                                dangling_events,
                                &anchor,
                            ),
                            SpanSet::SharedLocalSpans(local_spans) => amend_local_span(
                                local_spans,
                                trace_id,
                                parent_id,
                                committed_records,
                                dangling_events,
                                &anchor,
                            ),
                        },
                    }
                }

                mount_events(&mut committed_records[committed_len..], dangling_events);
                dangling_events.clear();
            }
        }

        if self.last_report.elapsed() > self.config.batch_report_interval
            || committed_records.len() > self.config.batch_report_max_spans.unwrap_or(usize::MAX)
            || flush
        {
            self.reporter
                .as_mut()
                .unwrap()
                .report(committed_records.drain(..).as_slice());
            self.last_report = std::time::Instant::now();
        }
    }
}

impl LocalSpansInner {
    pub fn to_span_records(&self, parent: SpanContext) -> Vec<SpanRecord> {
        let anchor: Anchor = Anchor::new();
        let mut dangling_events = HashMap::new();
        let mut records = Vec::new();
        amend_local_span(
            self,
            parent.trace_id,
            parent.span_id,
            &mut records,
            &mut dangling_events,
            &anchor,
        );
        mount_events(&mut records, &mut dangling_events);
        records
    }
}

fn amend_local_span(
    local_spans: &LocalSpansInner,
    trace_id: TraceId,
    parent_id: SpanId,
    spans: &mut Vec<SpanRecord>,
    events: &mut HashMap<SpanId, Vec<EventRecord>>,
    anchor: &Anchor,
) {
    for span in local_spans.spans.iter() {
        let begin_time_unix_ns = span.begin_instant.as_unix_nanos(anchor);
        let parent_id = if span.parent_id == SpanId::default() {
            parent_id
        } else {
            span.parent_id
        };

        if span.is_event {
            let event = EventRecord {
                name: span.name.clone(),
                timestamp_unix_ns: begin_time_unix_ns,
                properties: span.properties.clone(),
            };
            events.entry(parent_id).or_default().push(event);
            continue;
        }

        let end_time_unix_ns = if span.end_instant == span.begin_instant {
            local_spans.end_time.as_unix_nanos(anchor)
        } else {
            span.end_instant.as_unix_nanos(anchor)
        };
        spans.push(SpanRecord {
            trace_id,
            span_id: span.id,
            parent_id,
            begin_time_unix_ns,
            duration_ns: end_time_unix_ns.saturating_sub(begin_time_unix_ns),
            name: span.name.clone(),
            properties: span.properties.clone(),
            events: vec![],
        });
    }
}

fn amend_span(
    raw_span: &RawSpan,
    trace_id: TraceId,
    parent_id: SpanId,
    spans: &mut Vec<SpanRecord>,
    events: &mut HashMap<SpanId, Vec<EventRecord>>,
    anchor: &Anchor,
) {
    let begin_time_unix_ns = raw_span.begin_instant.as_unix_nanos(anchor);

    if raw_span.is_event {
        let event = EventRecord {
            name: raw_span.name.clone(),
            timestamp_unix_ns: begin_time_unix_ns,
            properties: raw_span.properties.clone(),
        };
        events.entry(parent_id).or_default().push(event);
        return;
    }

    let end_time_unix_ns = raw_span.end_instant.as_unix_nanos(anchor);
    spans.push(SpanRecord {
        trace_id,
        span_id: raw_span.id,
        parent_id,
        begin_time_unix_ns,
        duration_ns: end_time_unix_ns.saturating_sub(begin_time_unix_ns),
        name: raw_span.name.clone(),
        properties: raw_span.properties.clone(),
        events: vec![],
    });
}

fn mount_events(
    records: &mut [SpanRecord],
    dangling_events: &mut HashMap<SpanId, Vec<EventRecord>>,
) {
    for record in records.iter_mut() {
        if dangling_events.is_empty() {
            return;
        }

        if let Some(event) = dangling_events.remove(&record.span_id) {
            if record.events.is_empty() {
                record.events = event;
            } else {
                record.events.extend(event);
            }
        }
    }
}

impl SpanSet {
    fn len(&self) -> usize {
        match self {
            SpanSet::LocalSpansInner(local_spans) => local_spans.spans.len(),
            SpanSet::SharedLocalSpans(local_spans) => local_spans.spans.len(),
            SpanSet::Span(_) => 1,
        }
    }
}
