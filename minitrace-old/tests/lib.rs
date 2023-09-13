// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::Arc;

use futures::executor::block_on;

use minitrace::local::LocalCollector;
use minitrace::prelude::*;
use minitrace::util::tree::tree_str_from_span_records;
use tokio::runtime::Builder;

fn four_spans() {
    {
        // wide
        for _ in 0..2 {
            let mut span = LocalSpan::enter_with_local_parent("iter-span");
            span.add_property(|| ("tmp_property", "tmp_value".into()));
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
fn single_thread_single_span() {
    let collector = {
        let (root_span, collector) = Span::root("root");
        let _g = root_span.set_local_parent();

        four_spans();

        collector
    };

    let spans = block_on(collector.collect());

    let expected_graph = r#"
root []
    iter-span [("tmp_property", "tmp_value")]
    iter-span [("tmp_property", "tmp_value")]
    rec-span []
        rec-span []
"#;
    assert_eq!(tree_str_from_span_records(spans), expected_graph);
}

#[test]
fn single_thread_multiple_spans() {
    let (spans1, spans2, spans3) = {
        let (c1, c2, c3) = {
            let (root_span1, collector1) = Span::root("root1");
            let (root_span2, collector2) = Span::root("root2");
            let (root_span3, collector3) = Span::root("root3");

            let local_collector = LocalCollector::start();

            four_spans();

            let local_spans = Arc::new(local_collector.collect());

            root_span1.push_child_spans(local_spans.clone());
            root_span2.push_child_spans(local_spans.clone());
            root_span3.push_child_spans(local_spans);

            (collector1, collector2, collector3)
        };

        (
            block_on(c1.collect()),
            block_on(c2.collect()),
            block_on(c3.collect()),
        )
    };

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
    assert_eq!(tree_str_from_span_records(spans1), expected_graph1);
    assert_eq!(tree_str_from_span_records(spans2), expected_graph2);
    assert_eq!(tree_str_from_span_records(spans3), expected_graph3);
}

#[test]
fn multiple_threads_single_span() {
    let collector = crossbeam::scope(|scope| {
        let (span, collector) = Span::root("root");
        let _g = span.set_local_parent();

        for _ in 0..4 {
            let child_span = Span::enter_with_local_parent("cross-thread");
            scope.spawn(move |_| {
                let _g = child_span.set_local_parent();
                four_spans();
            });
        }

        four_spans();

        collector
    })
    .unwrap();

    let spans = block_on(collector.collect());

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
    assert_eq!(tree_str_from_span_records(spans), expected_graph);
}

#[test]
fn multiple_threads_multiple_spans() {
    let (spans1, spans2) = {
        let (c1, c2) = crossbeam::scope(|scope| {
            let (root_span1, collector1) = Span::root("root1");
            let (root_span2, collector2) = Span::root("root2");
            let local_collector = LocalCollector::start();

            for _ in 0..4 {
                let merged =
                    Span::enter_with_parents("merged", vec![&root_span1, &root_span2].into_iter());
                let _g = merged.set_local_parent();
                let _local = LocalSpan::enter_with_local_parent("local");
                scope.spawn(move |_| {
                    let local_collector = LocalCollector::start();

                    four_spans();

                    let local_spans = Arc::new(local_collector.collect());
                    merged.push_child_spans(local_spans);
                });
            }

            four_spans();

            let local_spans = Arc::new(local_collector.collect());
            root_span1.push_child_spans(local_spans.clone());
            root_span2.push_child_spans(local_spans);
            (collector1, collector2)
        })
        .unwrap();

        (block_on(c1.collect()), block_on(c2.collect()))
    };

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
    assert_eq!(tree_str_from_span_records(spans1), expected_graph1);
    assert_eq!(tree_str_from_span_records(spans2), expected_graph2);
}

#[test]
fn multiple_spans_without_local_spans() {
    let (spans1, spans2) = {
        let (c1, c2, c3) = {
            let (root_span1, collector1) = Span::root("root1");
            let (root_span2, collector2) = Span::root("root2");
            let (root_span3, collector3) = Span::root("root3");

            let local_collector = LocalCollector::start();

            let local_spans = Arc::new(local_collector.collect());
            root_span1.push_child_spans(local_spans.clone());
            root_span2.push_child_spans(local_spans.clone());
            root_span3.push_child_spans(local_spans);

            (collector1, collector2, collector3)
        };

        drop(c3);
        (block_on(c1.collect()), block_on(c2.collect()))
    };

    assert_eq!(spans1.len(), 1);
    assert_eq!(spans2.len(), 1);
}

#[test]
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

    #[trace(name = "work", enter_on_poll = true)]
    async fn work(millis: &u64) {
        let _g = Span::enter_with_local_parent("work-inner");
        tokio::time::sleep(std::time::Duration::from_millis(*millis))
            .enter_on_poll("sleep")
            .await;
    }

    impl Bar {
        #[trace(name = "work2")]
        async fn work2(&self, millis: &u64) {
            let _g = Span::enter_with_local_parent("work-inner");
            tokio::time::sleep(std::time::Duration::from_millis(*millis))
                .enter_on_poll("sleep")
                .await;
        }
    }

    #[trace(name = "work3")]
    async fn work3<'a>(millis1: &'a u64, millis2: &u64) {
        let _g = Span::enter_with_local_parent("work-inner");
        tokio::time::sleep(std::time::Duration::from_millis(*millis1))
            .enter_on_poll("sleep")
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(*millis2))
            .enter_on_poll("sleep")
            .await;
    }

    let collector = {
        let (root, collector) = Span::root("root");
        let _g = root.set_local_parent();

        let runtime = Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();
        block_on(runtime.spawn(Bar.run(&100))).unwrap();
        block_on(runtime.spawn(Bar.work2(&100))).unwrap();
        block_on(runtime.spawn(work3(&100, &100))).unwrap();

        collector
    };

    let spans = block_on(collector.collect());

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
    assert_eq!(tree_str_from_span_records(spans), expected_graph);
}

#[test]
fn macro_example() {
    #[trace(name = "do_something")]
    fn do_something(i: u64) {
        std::thread::sleep(std::time::Duration::from_millis(i));
    }

    #[trace(name = "do_something_async")]
    async fn do_something_async(i: u64) {
        futures_timer::Delay::new(std::time::Duration::from_millis(i)).await;
    }

    let (root, collector) = Span::root("root");

    {
        let _g = root.set_local_parent();
        do_something(100);
        block_on(do_something_async(100));
    }

    drop(root);
    let spans = block_on(collector.collect());

    let expected_graph = r#"
root []
    do_something []
    do_something_async []
"#;
    assert_eq!(tree_str_from_span_records(spans), expected_graph);
}

#[test]
fn multiple_local_parent() {
    let collector = {
        let (root, collector) = Span::root("root");
        let _g = root.set_local_parent();
        let _g = LocalSpan::enter_with_local_parent("span1");
        let span2 = Span::enter_with_local_parent("span2");
        {
            let _g = span2.set_local_parent();
            let _g = LocalSpan::enter_with_local_parent("span3");
        }
        let _g = LocalSpan::enter_with_local_parent("span4");

        collector
    };

    let spans = block_on(collector.collect());

    let expected_graph = r#"
root []
    span1 []
        span2 []
            span3 []
        span4 []
"#;
    assert_eq!(tree_str_from_span_records(spans), expected_graph);
}

#[test]
fn early_local_collect() {
    let local_collector = LocalCollector::start();
    let _g1 = LocalSpan::enter_with_local_parent("span1");
    let _g2 = LocalSpan::enter_with_local_parent("span2");
    drop(_g2);
    let local_spans = Arc::new(local_collector.collect());

    let (root, collector) = Span::root("root");
    root.push_child_spans(local_spans);
    drop(root);

    let spans = block_on(collector.collect());

    let expected_graph = r#"
root []
    span1 []
        span2 []
"#;
    assert_eq!(tree_str_from_span_records(spans), expected_graph);
}

#[test]
fn max_span_count() {
    fn block_until_next_collect_loop() {
        let (_, collector) = Span::root("dummy");
        block_on(collector.collect());
    }

    #[trace(name = "recursive")]
    fn recursive(n: usize) {
        if n > 1 {
            recursive(n - 1);
        }
    }

    let collector = {
        let (root, collector) =
            Span::root_with_args("root", CollectArgs::default().max_span_count(Some(5)));

        {
            let _g = root.set_local_parent();
            recursive(3);
        }
        block_until_next_collect_loop();
        {
            let _g = root.set_local_parent();
            recursive(3);
        }
        {
            let _g = root.set_local_parent();
            recursive(3);
        }
        block_until_next_collect_loop();
        {
            let _g = root.set_local_parent();
            recursive(3);
        }

        collector
    };

    let spans = block_on(collector.collect());

    let expected_graph = r#"
root []
    recursive []
        recursive []
            recursive []
    recursive []
        recursive []
            recursive []
"#;
    assert_eq!(tree_str_from_span_records(spans), expected_graph);
}
