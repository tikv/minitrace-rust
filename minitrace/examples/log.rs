// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::io::Write;

use log::info;
use log_derive::logfn;
use log_derive::logfn_inputs;
use minitrace::collector::Config;
use minitrace::collector::ConsoleReporter;
use minitrace::prelude::*;
use minitrace::Event;

#[logfn_inputs(DEBUG)]
#[logfn(ok = "DEBUG", err = "ERROR")]
#[trace]
fn plus(a: u64, b: u64) -> Result<u64, std::io::Error> {
    Ok(a + b)
}

fn main() {
    minitrace::set_reporter(ConsoleReporter, Config::default());
    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            // Add a event to the current local span representing the log record
            Event::add_to_local_parent(record.level().as_str(), || {
                [("message".into(), record.args().to_string().into())]
            });

            // Output the log to stdout as usual
            writeln!(buf, "[{}] {}", record.level(), record.args())
        })
        .filter_level(log::LevelFilter::Debug)
        .init();

    {
        let parent = SpanContext::new(TraceId(rand::random()), SpanId::default());
        let root = Span::root("root", parent);
        let _span_guard = root.set_local_parent();

        info!("event in root span");

        let _local_span_guard = LocalSpan::enter_with_local_parent("child");

        info!("event in child span");

        plus(1, 2).unwrap();
    };

    minitrace::flush();
}
