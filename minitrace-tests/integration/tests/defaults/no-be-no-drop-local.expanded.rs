extern crate alloc;
use minitrace::trace;
use minitrace::prelude::*;
use test_utilities::*;
#[trace]
async fn test_async(a: u32) -> u32 {
    a
}
#[trace]
fn test_sync(a: u32) -> u32 {
    a
}
#[tokio::main]
async fn main() {
    let (reporter, records) = minitrace::collector::TestReporter::new();
    minitrace::set_reporter(reporter, minitrace::collector::Config::default());
    let root = Span::root("root", SpanContext::random());
    let _child_span = root.set_local_parent();
    let mut handles = ::alloc::vec::Vec::new();
    handles.push(tokio::spawn(test_async(1)));
    test_sync(2);
    futures::future::join_all(handles).await;
    drop(root);
    let _expected = r#"[
    SpanRecord {
        id: 1,
        parent_id: 0,
        begin_unix_time_ns: \d+,
        duration_ns: \d+,
        event: "root",
        properties: [],
    },
]"#;
    let _actual = normalize_spans(records.lock().clone());
    ();
}
