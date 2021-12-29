// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::prelude::*;
use minitrace_macro::trace;

fn func1(i: u64) {
    let _guard = LocalSpan::enter_with_local_parent("func1");
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

        let _sg1 = span.set_local_parent();
        let _sg2 = LocalSpan::enter_with_local_parent("a span")
            .with_property(|| ("a property", "a value".to_owned()));

        for i in 1..=10 {
            func1(i);
        }

        collector
    }
    .collect_with_args(CollectArgs::default().sync(true));

    // Report to Jaeger
    let bytes =
        minitrace_jaeger::encode("synchronous".to_owned(), rand::random(), 0, 0, &spans).unwrap();
    minitrace_jaeger::report("127.0.0.1:6831".parse().unwrap(), &bytes).ok();

    // Report to Datadog
    let bytes = minitrace_datadog::encode("synchronous", rand::random(), 0, 0, &spans).unwrap();
    minitrace_datadog::report_blocking("127.0.0.1:8126".parse().unwrap(), bytes).ok();
}
