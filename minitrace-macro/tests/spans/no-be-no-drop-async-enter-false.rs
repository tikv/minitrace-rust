use minitrace::trace;
use test_utilities::*;
// With `enter_on_poll = false`, `async` functions construct `Span` that is
// thread safe.
//
// With no block expression the span "a-span" is silently omitted.
// Reference:
// - https://github.com/tikv/minitrace-rust/issues/125
// - https://github.com/tikv/minitrace-rust/issues/126
#[trace( name = "a-span", enter_on_poll=false)]
async fn f(a: u32) -> u32 {
    a
}

#[tokio::main]
async fn main() {
    let (root, collector) = minitrace::Span::root("root");
    //{
    let _child_span = root.set_local_parent();
    f(1).await;
    //}
    //drop(child_span); //This is required when not using `{ ... }`
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
        event: "a-span",
        properties: [],
    },
]"#;
    let actual = normalize_spans(records);
    assert_eq_text!(expected, &actual);
}
