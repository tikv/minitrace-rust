// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use futures::executor::block_on;
use minitrace::prelude::*;
use test_utilities::*;

fn func1(i: u64) {
    let _guard = LocalSpan::enter_with_local_parent("func1");
    std::thread::sleep(std::time::Duration::from_millis(i));
    func2(i);
}

#[trace( name = "func2")]
fn func2(i: u64) {
    std::thread::sleep(std::time::Duration::from_millis(i));
}

fn main() {
    let collector = {
        let (span, collector) = Span::root("root");

        let _sg1 = span.set_local_parent();
        let mut sg2 = LocalSpan::enter_with_local_parent("a span");
        sg2.add_property(|| ("a property", "a value".to_owned()));

        for i in 1..=10 {
            func1(i);
        }

        collector
    };

    let records: Vec<SpanRecord> = block_on(collector.collect());

    let expected = r#"[
    SpanRecord {
        id: 1,
        parent_id: 0,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "root",
        properties: [],
    },
    SpanRecord {
        id: 2,
        parent_id: 1,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "a span",
        properties: [
            (
                "a property",
                "a value",
            ),
        ],
    },
    SpanRecord {
        id: 3,
        parent_id: 2,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func1",
        properties: [],
    },
    SpanRecord {
        id: 4,
        parent_id: 3,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func2",
        properties: [],
    },
    SpanRecord {
        id: 5,
        parent_id: 2,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func1",
        properties: [],
    },
    SpanRecord {
        id: 6,
        parent_id: 5,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func2",
        properties: [],
    },
    SpanRecord {
        id: 7,
        parent_id: 2,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func1",
        properties: [],
    },
    SpanRecord {
        id: 8,
        parent_id: 7,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func2",
        properties: [],
    },
    SpanRecord {
        id: 9,
        parent_id: 2,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func1",
        properties: [],
    },
    SpanRecord {
        id: 10,
        parent_id: 9,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func2",
        properties: [],
    },
    SpanRecord {
        id: 11,
        parent_id: 2,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func1",
        properties: [],
    },
    SpanRecord {
        id: 12,
        parent_id: 11,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func2",
        properties: [],
    },
    SpanRecord {
        id: 13,
        parent_id: 2,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func1",
        properties: [],
    },
    SpanRecord {
        id: 14,
        parent_id: 13,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func2",
        properties: [],
    },
    SpanRecord {
        id: 15,
        parent_id: 2,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func1",
        properties: [],
    },
    SpanRecord {
        id: 16,
        parent_id: 15,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func2",
        properties: [],
    },
    SpanRecord {
        id: 17,
        parent_id: 2,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func1",
        properties: [],
    },
    SpanRecord {
        id: 18,
        parent_id: 17,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func2",
        properties: [],
    },
    SpanRecord {
        id: 19,
        parent_id: 2,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func1",
        properties: [],
    },
    SpanRecord {
        id: 20,
        parent_id: 19,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func2",
        properties: [],
    },
    SpanRecord {
        id: 21,
        parent_id: 2,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func1",
        properties: [],
    },
    SpanRecord {
        id: 22,
        parent_id: 21,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "func2",
        properties: [],
    },
]"#;
    let actual = normalize_spans(records);
    assert_eq_text!(expected, &actual);
}
