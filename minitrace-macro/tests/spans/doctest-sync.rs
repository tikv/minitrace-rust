use futures::executor::block_on;
use minitrace::prelude::*;
use test_utilities::*;

// Implement documentation example as an integration test.
//
// Reference:
// - https://github.com/tikv/minitrace-rust/blob/master/minitrace/src/lib.rs#L178-L202

#[trace("do_something")]
fn do_something(i: u64) {
    std::thread::sleep(std::time::Duration::from_millis(i));
}

fn main() {
    let (root, collector) = Span::root("root");

    {
        let _g = root.set_local_parent();
        do_something(100);
    }

    drop(root);
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
        event: "do_something",
        properties: [],
    },
]"#;
    let actual = normalize_spans(records);
    assert_eq_text!(expected, &actual);
}
