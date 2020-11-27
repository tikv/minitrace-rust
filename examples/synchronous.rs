// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::{start_scope, start_span, Scope};
use minitrace_jaeger::Reporter;
use minitrace_macro::trace;

fn func1(i: u64) {
    let _guard = start_span("func1");
    std::thread::sleep(std::time::Duration::from_millis(i));
    func2(i);
}

#[trace("func2")]
fn func2(i: u64) {
    std::thread::sleep(std::time::Duration::from_millis(i));
}

fn main() {
    let spans = {
        let (scope, collector) = Scope::root("root");

        let _scope_guard = start_scope(&scope);
        let _span_guard =
            start_span("a span").with_property(|| ("a property", "a value".to_owned()));

        for i in 1..=10 {
            func1(i);
        }

        collector
    }
    .collect(true, None);

    let reporter = Reporter::new("127.0.0.1:6831".parse().unwrap(), "synchronous");
    reporter
        .report(rand::random(), 0, rand::random(), &spans)
        .ok();
}
