use minitrace::trace;
use test_utilities::*;

// Reference:
// - https://github.com/tikv/minitrace-rust/issues/122
#[trace("test-span")]
async fn f(a: u32) -> u32 {
    a
}

#[tokio::main]
async fn main() {
    let (root, collector) = minitrace::Span::root("root");
    let _child_span = minitrace::Span::enter_with_parent("test-span", &root);
    f(1).await;
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
]"#;
    let actual = normalize_spans(records);
    assert_eq_text!(expected, &actual);
}
