// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::io::Write;
use std::net::SocketAddr;

use futures::executor::block_on;
use log::info;
use minitrace::prelude::*;
use minitrace::Event;

fn main() {
    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            // Add a event to the current local span representing the log record
            Event::add_to_local_parent(record.level().as_str(), || {
                [("message", record.args().to_string())]
            });

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

    let bytes =
        minitrace_jaeger::encode(String::from("service name"), rand::random(), 0, 0, &spans)
            .expect("encode error");

    let socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 6831);
    minitrace_jaeger::report_blocking(socket, &bytes).expect("report error");
}
