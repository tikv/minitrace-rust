// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

//! Test using minitrace without "report" feature, which means the minitrace is
//! statically disabled

#[test]
fn main() {
    use minitrace::local::LocalCollector;
    use minitrace::prelude::*;

    let mut root = Span::root("root", SpanContext::new(TraceId(0), SpanId(0)));
    root.add_property(|| ("k", "v".to_string()));
    root.add_properties(|| [("k", "v".to_string())]);

    Event::add_to_parent("event", &root, || []);
    Event::add_to_local_parent("event", || []);

    let _g = root.set_local_parent();

    Event::add_to_local_parent("event", || []);

    let mut span1 = LocalSpan::enter_with_local_parent("span1");
    span1.add_property(|| ("k", "v".to_string()));
    span1.add_properties(|| [("k", "v".to_string())]);

    let _span2 = LocalSpan::enter_with_local_parent("span2");

    let local_collector = LocalCollector::start();
    let _ = LocalSpan::enter_with_local_parent("span3");
    let local_spans = local_collector.collect();

    let span3 = Span::enter_with_parent("span3", &root);
    let span4 = Span::enter_with_local_parent("span4");
    let span5 = Span::enter_with_parents("span5", [&root, &span3, &span4]);

    span5.push_child_spans(local_spans);

    root.cancel();
}
