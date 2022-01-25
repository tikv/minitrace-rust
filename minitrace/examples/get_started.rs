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

    const TRACE_ID: u64 = 42;
    const SPAN_ID_PREFIX: u32 = 42;
    const ROOT_PARENT_SPAN_ID: u64 = 0;
    let bytes = minitrace_jaeger::encode(
        String::from("service name"),
        TRACE_ID,
        ROOT_PARENT_SPAN_ID,
        SPAN_ID_PREFIX,
        &spans,
    )
    .expect("encode error");

    let socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 6831);
    minitrace_jaeger::report_blocking(socket, &bytes).expect("report error");
}
