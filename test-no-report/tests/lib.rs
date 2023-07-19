// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

// The libraries may have tracing instrument embedded in the code for tracing purposes. However,
// if the executable does not enable minitrace, it will be statically disabled. This results in
// zero overhead to the libraries, achieved through conditional compilation with the "report" feature.
//
// The following test is designed to confirm that minitrace compiles when it's statically disabled in the executable.

use std::time::Duration;

use minitrace::collector::Config;
use minitrace::collector::ConsoleReporter;

#[test]
fn test_no_report() {
    use minitrace::local::LocalCollector;
    use minitrace::prelude::*;

    minitrace::set_reporter(
        ConsoleReporter,
        Config::default()
            .batch_report_interval(Duration::from_secs(1))
            .max_spans_per_trace(Some(100))
            .batch_report_max_spans(Some(200)),
    );

    let mut root = Span::root("root", SpanContext::new(TraceId(0), SpanId(0)))
        .with_property(|| ("k1", "v1".to_string()))
        .with_properties(|| [("k2", "v2".to_string())]);

    Event::add_to_parent("event", &root, || []);
    Event::add_to_local_parent("event", || []);

    let _g = root.set_local_parent();

    Event::add_to_local_parent("event", || []);

    let _span1 = LocalSpan::enter_with_local_parent("span1")
        .with_property(|| ("k", "v".to_string()))
        .with_properties(|| [("k", "v".to_string())]);

    let _span2 = LocalSpan::enter_with_local_parent("span2");

    let local_collector = LocalCollector::start();
    let _ = LocalSpan::enter_with_local_parent("span3");
    let local_spans = local_collector.collect();

    let span3 = Span::enter_with_parent("span3", &root);
    let span4 = Span::enter_with_local_parent("span4");
    let span5 = Span::enter_with_parents("span5", [&root, &span3, &span4]);

    span5.push_child_spans(local_spans);

    root.cancel();

    minitrace::flush();
}
