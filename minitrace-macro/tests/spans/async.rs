use minitrace::trace;
use regex::Regex;
use test_utilities::*;

// This Tracing crate like-syntax
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
    let pre = format!("{records:#?}");
    let re1 = Regex::new(r"begin_unix_time_ns: \d+,").unwrap();
    let re2 = Regex::new(r"duration_ns: \d+,").unwrap();
    let int: std::string::String = re1.replace_all(&pre, r"begin_unix_time_ns: \d+,").into();
    let actual: std::string::String = re2.replace_all(&int, r"duration_ns: \d+,").into();
    assert_eq_text!(expected, &actual);
}
