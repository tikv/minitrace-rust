// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::Duration;

use minitrace::collector::Collected;
use minitrace::prelude::*;
use once_cell::sync::OnceCell;
use tokio::sync::mpsc::Sender;

static GLOBAL_REPORTER: OnceCell<Sender<(TraceContext, Collected)>> = OnceCell::new();
const INTERVAL: Duration = Duration::from_secs(1);
const MAX_BATCH_SIZE: usize = 100;

struct TraceContext {
    trace_id: u64,
    parent_id: u64,
}

pub fn init() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<(TraceContext, Collected)>(10000);

    // Spawn a background task to collect spans and report them to Jaeger
    tokio::spawn(async move {
        loop {
            let start = tokio::time::Instant::now();

            // Collect spans from the channel
            let mut jaeger_spans = Vec::new();
            while let Ok((trace_context, collected)) = receiver.try_recv() {
                let spans = collected.await;
                jaeger_spans.extend(minitrace_jaeger::convert(
                    &spans,
                    0,
                    trace_context.trace_id,
                    trace_context.parent_id,
                ));

                // Stop collecting and report if the batch is full
                if jaeger_spans.len() >= MAX_BATCH_SIZE {
                    break;
                }
            }

            // Report spans to Jaeger
            let span_count = jaeger_spans.len();
            minitrace_jaeger::report(
                "batch".to_string(),
                "127.0.0.1:6831".parse().unwrap(),
                jaeger_spans,
            )
            .await
            .expect("report error");

            // Sleep if the batch is not full
            if span_count < MAX_BATCH_SIZE {
                tokio::time::sleep_until(start + INTERVAL).await;
            }
        }
    });

    // Store the sender in a global variable
    GLOBAL_REPORTER
        .set(sender)
        .expect("global reporter already initialized");
}

/// Start a new tracing session for a request
pub fn start_tracing_request(
    name: &'static str,
    trace_id: u64,
    parent_id: u64,
) -> (Span, impl Drop) {
    let (span, collector) = Span::root(name);
    let defer = defer::defer(move || {
        GLOBAL_REPORTER
            .get()
            .unwrap()
            .try_send((
                TraceContext {
                    trace_id,
                    parent_id,
                },
                collector.collect(),
            ))
            .ok();
    });

    (span, defer)
}

#[tokio::main]
async fn main() {
    init();

    for _ in 0..1000 {
        let (root, _g) = start_tracing_request("get_user", rand::random(), 0);
        let _g = root.set_local_parent();

        let _g = LocalSpan::enter_with_local_parent("query_database");

        tokio::time::sleep(Duration::from_millis(2)).await;
    }

    // Wait for the reporter to finish the last batch
    tokio::time::sleep(Duration::from_secs(1)).await;
}
