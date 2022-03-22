use minitrace::trace;
use test_utilities::*;

#[trace]
async fn fa(a: u32) -> u32 {
    a
}

#[tokio::main]
async fn main() {
    let (root, collector) = minitrace::Span::root("root");
    {
        let _g = root.set_local_parent();
        fa(1).await;
    }
    drop(root);
    let records: Vec<minitrace::collector::SpanRecord> = futures::executor::block_on(collector.collect());

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
        event: "fa",
        properties: [],
    },
]"#;
    let actual = normalize_spans(records);
    assert_eq_text!(expected, &actual);
}
