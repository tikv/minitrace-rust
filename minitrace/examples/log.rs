// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::io::Write;
use std::net::SocketAddr;

use futures::executor::block_on;
use log::info;
use log_derive::logfn;
use log_derive::logfn_inputs;
use minitrace::prelude::*;
use minitrace::Event;

#[logfn_inputs(DEBUG)]
#[logfn(ok = "DEBUG", err = "ERROR")]
#[trace]
fn plus(a: u64, b: u64) -> Result<u64, std::io::Error> {
    Ok(a + b)
}

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
        .filter_level(log::LevelFilter::Debug)
        .init();

    let collector = {
        let (root_span, collector) = Span::root("root");
        let _span_guard = root_span.set_local_parent();

        info!("event in root span");

        let _local_span_guard = LocalSpan::enter_with_local_parent("child");

        info!("event in child span");

        plus(1, 2).unwrap();

        collector
    };

    let spans = block_on(collector.collect());

    let jaeger_spans = minitrace_jaeger::convert(&spans, 0, rand::random(), 0).collect();

    let socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 6831);
    minitrace_jaeger::report_blocking("log".to_string(), socket, jaeger_spans)
        .expect("report error");
}
