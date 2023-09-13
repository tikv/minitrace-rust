use minitrace::trace;
use test_utilities::*;

// With default (`enter_on_poll = false`), `async` functions construct `
// Span` that is thread safe.

// With no block expression the span "test-span" is silently omitted.
// Reference:
// - https://github.com/tikv/minitrace-rust/issues/125
// - https://github.com/tikv/minitrace-rust/issues/126
#[trace]
async fn test_async(a: u32) -> u32 {
    a
}

#[trace]
fn test_sync(a: u32) -> u32 {
    a
}

#[tokio::main]
//#[trace( name = "start", root=true, reporter=None)]
// reporter: Datadog, Jaeger, None (default)
fn main() {
    //let minitrace = minitrace::Trace::new("name", Local )
    let (root, collector) = minitrace::Span::root("start");
    //let child_span = minitrace.new("test-span", &root);
    let child_span = minitrace::Span::enter_with_parent("test-span", &root);

    let mut handles = vec![];
    handles.push(tokio::spawn(test_async(1).await));
    test_sync(2);
    //minitrace.record("key", "Value");

    futures::future::join_all(handles).await;

    //drop(minitrace.spans);
    //}
    //drop(child_span);
    drop(root);
    // let records: minitrace.collect()
    let records: Vec<minitrace::collector::SpanRecord> =
        futures::executor::block_on(collector.collect());
    // let minitrace.report() // when reporter is not None (default)
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
