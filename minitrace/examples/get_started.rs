// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use std::net::SocketAddr;

use futures::executor::block_on;
use minitrace::prelude::*;

fn main() {
    let collector = {
        let (root_span, collector) = Span::root("root");
        let _span_guard = root_span.set_local_parent();

        let _local_span_guard = LocalSpan::enter_with_local_parent("child");

        // do something ...
        collector
    };

    let spans = block_on(collector.collect());

    const NODE_ID: u32 = 42;
    const TRACE_ID: u64 = 42;
    const ROOT_PARENT_SPAN_ID: u64 = 0;
    let jaeger_spans = minitrace_jaeger::convert(&spans, NODE_ID, TRACE_ID, ROOT_PARENT_SPAN_ID)
        .collect::<Vec<_>>();

    let socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 6831);
    minitrace_jaeger::report_blocking("service name".to_string(), socket, jaeger_spans)
        .expect("report error");
}
