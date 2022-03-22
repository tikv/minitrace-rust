use minitrace::trace;
use test_utilities::*;

// Span names passed via `enter_with_parent` override default names.
// Reference:
// - https://github.com/tikv/minitrace-rust/issues/122
#[trace("a-span")]
fn f(a: u32) -> u32 {
    a
}

fn main() {
    let (root, collector) = minitrace::Span::root("root");
    {
        let _sg1 = minitrace::Span::enter_with_parent("test-span", &root);
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
        event: "test-span",
        properties: [],
    },
]"#;
    let actual = normalize_spans(records);
    assert_eq_text!(expected, &actual);
}
