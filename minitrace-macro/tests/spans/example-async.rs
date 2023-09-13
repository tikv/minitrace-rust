// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use futures::executor::block_on;
use minitrace::prelude::*;
use test_utilities::*;

fn parallel_job() -> Vec<tokio::task::JoinHandle<()>> {
    let mut v = Vec::with_capacity(4);
    for i in 0..4 {
        v.push(tokio::spawn(
            iter_job(i).in_span(Span::enter_with_local_parent("iter job")),
        ));
    }
    v
}

async fn iter_job(iter: u64) {
    std::thread::sleep(std::time::Duration::from_millis(iter * 10));
    tokio::task::yield_now().await;
    other_job().await;
}

#[trace( name = "other job", enter_on_poll = true)]
async fn other_job() {
    for i in 0..20 {
        if i == 10 {
            tokio::task::yield_now().await;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[tokio::main]
async fn main() {
    let (span, collector) = Span::root("root");

    let f = async {
        let jhs = {
            let mut span = LocalSpan::enter_with_local_parent("a span");
            span.add_property(|| ("a property", "a value".to_owned()));
            parallel_job()
        };

        other_job().await;

        for jh in jhs {
            jh.await.unwrap();
        }
    }
    .in_span(span);

    tokio::spawn(f).await.unwrap();

    let records: Vec<SpanRecord> = block_on(collector.collect());

    let expected = r#"[
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
    SpanRecord {
        id: \d+,
        parent_id: \d+,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "...",
        properties: [ ... ],
    },
]"#;
    let actual = normalize_async_spans(records);
    assert_eq_text!(expected, &actual);
}
