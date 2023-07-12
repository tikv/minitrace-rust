// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::io::Write;
use std::net::SocketAddr;

use futures::executor::block_on;
use log::info;
use minitrace::prelude::*;

fn main() {
    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            // Add a local span to represent the log record
            let mut span = LocalSpan::enter_with_local_parent(record.level().as_str());
            span.add_property(|| ("message", record.args().to_string()));

            // Output the log to stdout as usual
            writeln!(buf, "[{}] {}", record.level(), record.args())
        })
        .filter_level(log::LevelFilter::Info)
        .init();

    let collector = {
        let (root_span, collector) = Span::root("root");
        let _span_guard = root_span.set_local_parent();

        info!("event in root span");

        let _local_span_guard = LocalSpan::enter_with_local_parent("child");

        info!("event in child span");

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
