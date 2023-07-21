// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use futures::executor::block_on;
use minitrace::collector::Config;
use minitrace::collector::TestReporter;
use minitrace::local::LocalCollector;
use minitrace::prelude::*;
use minitrace::util::tree::tree_str_from_span_records;
use serial_test::serial;
use tokio::runtime::Builder;

fn four_spans() {
    {
        // wide
        for _ in 0..2 {
            let _span = LocalSpan::enter_with_local_parent("iter-span")
                .with_property(|| ("tmp_property".into(), "tmp_value".into()));
        }
    }

    {
        #[trace(name = "rec-span")]
        fn rec(mut i: u32) {
            i -= 1;

            if i > 0 {
                rec(i);
            }
        }

        // deep
        rec(2);
    }
}

#[test]
#[serial]
fn single_thread_single_span() {
    let (reporter, collected_spans) = TestReporter::new();
    minitrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
        let _g = root.set_local_parent();

        four_spans();
    };

    minitrace::flush();

    let expected_graph = r#"
root []
    iter-span [("tmp_property", "tmp_value")]
    iter-span [("tmp_property", "tmp_value")]
    rec-span []
        rec-span []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
}

#[test]
#[serial]
fn single_thread_multiple_spans() {
    let (reporter, collected_spans) = TestReporter::new();
    minitrace::set_reporter(reporter, Config::default());

    {
        let root1 = Span::root("root1", SpanContext::new(TraceId(12), SpanId::default()));
        let root2 = Span::root("root2", SpanContext::new(TraceId(13), SpanId::default()));
        let root3 = Span::root("root3", SpanContext::new(TraceId(14), SpanId::default()));

        let local_collector = LocalCollector::start();

        four_spans();

        let local_spans = local_collector.collect();

        root1.push_child_spans(local_spans.clone());
        root2.push_child_spans(local_spans.clone());
        root3.push_child_spans(local_spans);
    }

    minitrace::flush();

    let expected_graph1 = r#"
root1 []
    iter-span [("tmp_property", "tmp_value")]
    iter-span [("tmp_property", "tmp_value")]
    rec-span []
        rec-span []
"#;
    let expected_graph2 = r#"
root2 []
    iter-span [("tmp_property", "tmp_value")]
    iter-span [("tmp_property", "tmp_value")]
    rec-span []
        rec-span []
"#;
    let expected_graph3 = r#"
root3 []
    iter-span [("tmp_property", "tmp_value")]
    iter-span [("tmp_property", "tmp_value")]
    rec-span []
        rec-span []
"#;
    assert_eq!(
        tree_str_from_span_records(
            collected_spans
                .lock()
                .iter()
                .filter(|s| s.trace_id == TraceId(12))
                .cloned()
                .collect()
        ),
        expected_graph1
    );
    assert_eq!(
        tree_str_from_span_records(
            collected_spans
                .lock()
                .iter()
                .filter(|s| s.trace_id == TraceId(13))
                .cloned()
                .collect()
        ),
        expected_graph2
    );
    assert_eq!(
        tree_str_from_span_records(
            collected_spans
                .lock()
                .iter()
                .filter(|s| s.trace_id == TraceId(14))
                .cloned()
                .collect()
        ),
        expected_graph3
    );
}

#[test]
#[serial]
fn multiple_threads_single_span() {
    let (reporter, collected_spans) = TestReporter::new();
    minitrace::set_reporter(reporter, Config::default());

    crossbeam::scope(|scope| {
        let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
        let _g = root.set_local_parent();

        for _ in 0..4 {
            let child_span = Span::enter_with_local_parent("cross-thread");
            scope.spawn(move |_| {
                let _g = child_span.set_local_parent();
                four_spans();
            });
        }

        four_spans();
    })
    .unwrap();

    minitrace::flush();

    let expected_graph = r#"
root []
    cross-thread []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        rec-span []
            rec-span []
    cross-thread []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        rec-span []
            rec-span []
    cross-thread []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        rec-span []
            rec-span []
    cross-thread []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        rec-span []
            rec-span []
    iter-span [("tmp_property", "tmp_value")]
    iter-span [("tmp_property", "tmp_value")]
    rec-span []
        rec-span []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
}

#[test]
#[serial]
fn multiple_threads_multiple_spans() {
    let (reporter, collected_spans) = TestReporter::new();
    minitrace::set_reporter(reporter, Config::default());

    crossbeam::scope(|scope| {
        let root1 = Span::root("root1", SpanContext::new(TraceId(12), SpanId::default()));
        let root2 = Span::root("root2", SpanContext::new(TraceId(13), SpanId::default()));
        let local_collector = LocalCollector::start();

        for _ in 0..4 {
            let merged = Span::enter_with_parents("merged", vec![&root1, &root2]);
            let _g = merged.set_local_parent();
            let _local = LocalSpan::enter_with_local_parent("local");
            scope.spawn(move |_| {
                let local_collector = LocalCollector::start();

                four_spans();

                let local_spans = local_collector.collect();
                merged.push_child_spans(local_spans);
            });
        }

        four_spans();

        let local_spans = local_collector.collect();
        root1.push_child_spans(local_spans.clone());
        root2.push_child_spans(local_spans);
    })
    .unwrap();

    minitrace::flush();

    let expected_graph1 = r#"
root1 []
    iter-span [("tmp_property", "tmp_value")]
    iter-span [("tmp_property", "tmp_value")]
    merged []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        local []
        rec-span []
            rec-span []
    merged []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        local []
        rec-span []
            rec-span []
    merged []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        local []
        rec-span []
            rec-span []
    merged []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        local []
        rec-span []
            rec-span []
    rec-span []
        rec-span []
"#;
    let expected_graph2 = r#"
root2 []
    iter-span [("tmp_property", "tmp_value")]
    iter-span [("tmp_property", "tmp_value")]
    merged []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        local []
        rec-span []
            rec-span []
    merged []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        local []
        rec-span []
            rec-span []
    merged []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        local []
        rec-span []
            rec-span []
    merged []
        iter-span [("tmp_property", "tmp_value")]
        iter-span [("tmp_property", "tmp_value")]
        local []
        rec-span []
            rec-span []
    rec-span []
        rec-span []
"#;
    assert_eq!(
        tree_str_from_span_records(
            collected_spans
                .lock()
                .iter()
                .filter(|s| s.trace_id == TraceId(12))
                .cloned()
                .collect()
        ),
        expected_graph1
    );
    assert_eq!(
        tree_str_from_span_records(
            collected_spans
                .lock()
                .iter()
                .filter(|s| s.trace_id == TraceId(13))
                .cloned()
                .collect()
        ),
        expected_graph2
    );
}

#[test]
#[serial]
fn multiple_spans_without_local_spans() {
    let (reporter, collected_spans) = TestReporter::new();
    minitrace::set_reporter(reporter, Config::default());

    {
        let root1 = Span::root("root1", SpanContext::new(TraceId(12), SpanId::default()));
        let root2 = Span::root("root2", SpanContext::new(TraceId(13), SpanId::default()));
        let mut root3 = Span::root("root3", SpanContext::new(TraceId(14), SpanId::default()));

        let local_collector = LocalCollector::start();

        let local_spans = local_collector.collect();
        root1.push_child_spans(local_spans.clone());
        root2.push_child_spans(local_spans.clone());
        root3.push_child_spans(local_spans);

        root3.cancel();
    }

    minitrace::flush();

    assert_eq!(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(12))
            .count(),
        1
    );
    assert_eq!(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(13))
            .count(),
        1
    );
    assert_eq!(
        collected_spans
            .lock()
            .iter()
            .filter(|s| s.trace_id == TraceId(14))
            .count(),
        0
    );
}

#[test]
#[serial]
fn test_macro() {
    use async_trait::async_trait;

    #[async_trait]
    trait Foo {
        async fn run(&self, millis: &u64);
    }

    struct Bar;

    #[async_trait]
    impl Foo for Bar {
        #[trace(name = "run")]
        async fn run(&self, millis: &u64) {
            let _g = Span::enter_with_local_parent("run-inner");
            work(millis).await;
            let _g = LocalSpan::enter_with_local_parent("local-span");
        }
    }

    #[trace(enter_on_poll = true)]
    async fn work(millis: &u64) {
        let _g = Span::enter_with_local_parent("work-inner");
        tokio::time::sleep(std::time::Duration::from_millis(*millis))
            .enter_on_poll("sleep")
            .await;
    }

    impl Bar {
        #[trace]
        async fn work2(&self, millis: &u64) {
            let _g = Span::enter_with_local_parent("work-inner");
            tokio::time::sleep(std::time::Duration::from_millis(*millis))
                .enter_on_poll("sleep")
                .await;
        }
    }

    #[trace]
    async fn work3<'a>(millis1: &'a u64, millis2: &u64) {
        let _g = Span::enter_with_local_parent("work-inner");
        tokio::time::sleep(std::time::Duration::from_millis(*millis1))
            .enter_on_poll("sleep")
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(*millis2))
            .enter_on_poll("sleep")
            .await;
    }

    let (reporter, collected_spans) = TestReporter::new();
    minitrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));

        let runtime = Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();

        block_on(
            runtime.spawn(
                async {
                    Bar.run(&100).await;
                    Bar.work2(&100).await;
                    work3(&100, &100).await;
                }
                .in_span(root),
            ),
        )
        .unwrap();
    }

    minitrace::flush();

    let expected_graph = r#"
root []
    run []
        local-span []
        run-inner []
        work []
            sleep []
        work []
            sleep []
            work-inner []
    work2 []
        sleep []
        sleep []
        work-inner []
    work3 []
        sleep []
        sleep []
        sleep []
        sleep []
        work-inner []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
}

#[test]
#[serial]
fn macro_example() {
    #[trace]
    fn do_something(i: u64) {
        std::thread::sleep(std::time::Duration::from_millis(i));
    }

    #[trace]
    async fn do_something_async(i: u64) {
        futures_timer::Delay::new(std::time::Duration::from_millis(i)).await;
    }

    let (reporter, collected_spans) = TestReporter::new();
    minitrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
        let _g = root.set_local_parent();
        do_something(100);
        block_on(do_something_async(100));
    }

    minitrace::flush();

    let expected_graph = r#"
root []
    do_something []
    do_something_async []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
}

#[test]
#[serial]
fn multiple_local_parent() {
    let (reporter, collected_spans) = TestReporter::new();
    minitrace::set_reporter(reporter, Config::default());

    {
        let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
        let _g = root.set_local_parent();
        let _g = LocalSpan::enter_with_local_parent("span1");
        let span2 = Span::enter_with_local_parent("span2");
        {
            let _g = span2.set_local_parent();
            let _g = LocalSpan::enter_with_local_parent("span3");
        }
        let _g = LocalSpan::enter_with_local_parent("span4");
    }

    minitrace::flush();

    let expected_graph = r#"
root []
    span1 []
        span2 []
            span3 []
        span4 []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
}

#[test]
#[serial]
fn early_local_collect() {
    let (reporter, collected_spans) = TestReporter::new();
    minitrace::set_reporter(reporter, Config::default());

    {
        let local_collector = LocalCollector::start();
        let _g1 = LocalSpan::enter_with_local_parent("span1");
        let _g2 = LocalSpan::enter_with_local_parent("span2");
        drop(_g2);
        let local_spans = local_collector.collect();

        let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
        root.push_child_spans(local_spans);
    }

    minitrace::flush();

    let expected_graph = r#"
root []
    span1 []
        span2 []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
}

#[test]
#[serial]
fn max_spans_per_trace() {
    #[trace]
    fn recursive(n: usize) {
        if n > 1 {
            recursive(n - 1);
        }
    }

    let (reporter, collected_spans) = TestReporter::new();
    minitrace::set_reporter(reporter, Config::default().max_spans_per_trace(Some(5)));

    {
        let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));

        {
            let _g = root.set_local_parent();
            recursive(3);
        }
        {
            let _g = root.set_local_parent();
            recursive(3);
        }
        {
            let _g = root.set_local_parent();
            recursive(3);
        }
        {
            let _g = root.set_local_parent();
            recursive(3);
        }
    }

    minitrace::flush();

    let expected_graph = r#"
root []
    recursive []
        recursive []
            recursive []
    recursive []
        recursive []
            recursive []
"#;
    assert_eq!(
        tree_str_from_span_records(collected_spans.lock().clone()),
        expected_graph
    );
}
