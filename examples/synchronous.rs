// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::CollectArgs;
use minitrace::{LocalSpan, Span};
use minitrace_datadog::Reporter as DReporter;
use minitrace_jaeger::Reporter as JReporter;
use minitrace_macro::trace;

fn func1(i: u64) {
    let _guard = LocalSpan::enter("func1");
    std::thread::sleep(std::time::Duration::from_millis(i));
    func2(i);
}

#[trace("func2")]
fn func2(i: u64) {
    std::thread::sleep(std::time::Duration::from_millis(i));
}

fn main() {
    let spans = {
        let (span, collector) = Span::root("root");

        let _sg1 = span.enter();
        let _sg2 =
            LocalSpan::enter("a span").with_property(|| ("a property", "a value"));

        for i in 1..=10 {
            func1(i);
        }

        collector
    }
    .collect_with_args(CollectArgs::default().sync(true));

    // Report to Jaeger
    let bytes = JReporter::encode("synchronous".to_owned(), rand::random(), 0, 0, &spans).unwrap();
    JReporter::report("127.0.0.1:6831".parse().unwrap(), &bytes).ok();

    // Report to Datadog
    let bytes = DReporter::encode("synchronous", "http", "GET /", 0, rand::random(), 0, 0, &spans).unwrap();
    DReporter::report_blocking("127.0.0.1:8126".parse().unwrap(), bytes).ok();
}
