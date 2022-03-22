use minitrace::trace;
use test_utilities::*;

#[trace]
fn f(a: u64) {
    std::thread::sleep(std::time::Duration::from_nanos(a));
}

fn main() {
    let (root, collector) = minitrace::Span::root("root");
    {
        let _g = root.set_local_parent();
        f(1);
    }
    drop(root);
    let records: Vec<minitrace::collector::SpanRecord> =
        futures::executor::block_on(collector.collect());
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
        event: "f",
        properties: [],
    },
]"#;
    let actual = normalize_spans(records);
    assert_eq_text!(expected, &actual);
}
