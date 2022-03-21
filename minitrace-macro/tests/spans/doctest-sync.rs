use futures::executor::block_on;
use minitrace::prelude::*;
use regex::Regex;
use test_utilities::*;

// Implement doctest example as an integration test.
//
// Reference:
// - https://github.com/tikv/minitrace-rust/blob/master/minitrace/src/lib.rs#L178-L202

#[trace("do_something")]
fn do_something(i: u64) {
    std::thread::sleep(std::time::Duration::from_millis(i));
}

// #[trace("do_something_async")]
// async fn do_something_async(i: u64) {
//     futures_timer::Delay::new(std::time::Duration::from_millis(i)).await;
// }

fn main() {
    let (root, collector) = Span::root("root");

    {
        let _g = root.set_local_parent();
        do_something(100);
        //    block_on(do_something_async(100));
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
    let pre = format!("{records:#?}");
    let re1 = Regex::new(r"begin_unix_time_ns: \d+,").unwrap();
    let re2 = Regex::new(r"duration_ns: \d+,").unwrap();
    let int: std::string::String = re1.replace_all(&pre, r"begin_unix_time_ns: \d+,").into();
    let actual: std::string::String = re2.replace_all(&int, r"duration_ns: \d+,").into();
    assert_eq_text!(expected, &actual);

}
