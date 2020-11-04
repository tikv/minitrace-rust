// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::{new_span, root_scope};
use minitrace_jaeger::Reporter;
use minitrace_macro::trace;
use std::net::{Ipv4Addr, SocketAddr};

fn func1(i: u64) {
    let _guard = new_span("func1");
    std::thread::sleep(std::time::Duration::from_millis(i));
    func2(i);
}

#[trace("func2")]
fn func2(i: u64) {
    std::thread::sleep(std::time::Duration::from_millis(i));
}

fn main() {
    let spans = {
        let (scope, collector) = root_scope("root");

        let _scope_guard = scope.start_scope();
        let _span_guard = new_span("a span").with_property(|| ("a property", "a value".to_owned()));

        for i in 1..=10 {
            func1(i);
        }

        collector
    }
    .collect(true, None, None);

    let socket = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 6831);
    let reporter = Reporter::new(socket, "synchronous");
    reporter.report(rand::random(), spans).ok();
}
