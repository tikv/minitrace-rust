// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::Arc;

use futures::executor::block_on;

use minitrace::local::LocalCollector;
use minitrace::prelude::*;
use tokio::runtime::Builder;

fn four_spans() {
    {
        // wide
        for _ in 0..2 {
            let _g = LocalSpan::enter_with_local_parent("iter-span")
                .with_property(|| ("tmp_property", "tmp_value".into()));
        }
    }

    {
        #[trace("rec-span")]
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
root
    rec-span
        rec-span
    iter-span
    iter-span
"#;
    assert_graph(spans, expected_graph);
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
root1
    rec-span
        rec-span
    iter-span
    iter-span
"#;
    let expected_graph2 = r#"
root2
    rec-span
        rec-span
    iter-span
    iter-span
"#;
    let expected_graph3 = r#"
root3
    rec-span
        rec-span
    iter-span
    iter-span
"#;
    assert_graph(spans1, expected_graph1);
    assert_graph(spans2, expected_graph2);
    assert_graph(spans3, expected_graph3);
}

#[test]
fn multiple_threads_single_span() {
    let collector = {
        let (span, collector) = Span::root("root");
        let _g = span.set_local_parent();

        for _ in 0..4 {
            let child_span = Span::enter_with_local_parent("cross-thread");
            std::thread::spawn(move || {
                let _g = child_span.set_local_parent();
                four_spans();
            });
        }

        four_spans();

        collector
    };

    let spans = block_on(collector.collect());

    let expected_graph = r#"
root
    rec-span
        rec-span
    iter-span
    iter-span
    cross-thread
        rec-span
            rec-span
        iter-span
        iter-span
    cross-thread
        rec-span
            rec-span
        iter-span
        iter-span
    cross-thread
        rec-span
            rec-span
        iter-span
        iter-span
    cross-thread
        rec-span
            rec-span
        iter-span
        iter-span
"#;
    assert_graph(spans, expected_graph);
}

#[test]
fn multiple_threads_multiple_spans() {
    let (spans1, spans2) = {
        let (c1, c2) = {
            let (root_span1, collector1) = Span::root("root1");
            let (root_span2, collector2) = Span::root("root2");
            let local_collector = LocalCollector::start();

            for _ in 0..4 {
                let merged =
                    Span::enter_with_parents("merged", vec![&root_span1, &root_span2].into_iter());
                std::thread::spawn(move || {
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
        };

        (block_on(c1.collect()), block_on(c2.collect()))
    };

    let expected_graph1 = r#"
root1
    rec-span
        rec-span
    merged
        rec-span
            rec-span
        iter-span
        iter-span
    merged
        rec-span
            rec-span
        iter-span
        iter-span
    merged
        rec-span
            rec-span
        iter-span
        iter-span
    merged
        rec-span
            rec-span
        iter-span
        iter-span
    iter-span
    iter-span
"#;
    let expected_graph2 = r#"
root2
    rec-span
        rec-span
    merged
        rec-span
            rec-span
        iter-span
        iter-span
    merged
        rec-span
            rec-span
        iter-span
        iter-span
    merged
        rec-span
            rec-span
        iter-span
        iter-span
    merged
        rec-span
            rec-span
        iter-span
        iter-span
    iter-span
    iter-span
"#;
    assert_graph(spans1, expected_graph1);
    assert_graph(spans2, expected_graph2);
}

#[test]
fn multiple_spans_without_local_spans() {
    let (spans1, spans2, spans3) = {
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

        (
            block_on(c1.collect()),
            block_on(c2.collect()),
            block_on(c3.collect()),
        )
    };

    assert_eq!(spans1.len(), 1);
    assert_eq!(spans2.len(), 1);
    assert_eq!(spans3.len(), 1);
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
        #[trace("run")]
        async fn run(&self, millis: &u64) {
            let _g = Span::enter_with_local_parent("run-inner");
            work(millis).await;
            let _g = LocalSpan::enter_with_local_parent("local-span");
        }
    }

    #[trace("work", enter_on_poll = true)]
    async fn work(millis: &u64) {
        let _g = Span::enter_with_local_parent("work-inner");
        tokio::time::sleep(std::time::Duration::from_millis(*millis))
            .enter_on_poll("sleep")
            .await;
    }

    impl Bar {
        #[trace("work2")]
        async fn work2(&self, millis: &u64) {
            let _g = Span::enter_with_local_parent("work-inner");
            tokio::time::sleep(std::time::Duration::from_millis(*millis))
                .enter_on_poll("sleep")
                .await;
        }
    }

    #[trace("work3")]
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
root
    work3
        work-inner
        sleep
        sleep
        sleep
        sleep
    work2
        work-inner
        sleep
        sleep
    run
        work
            sleep
        work
            work-inner
            sleep
        run-inner
        local-span
"#;
    assert_graph(spans, expected_graph);
}

#[test]
fn macro_example() {
    #[trace("do_something")]
    fn do_something(i: u64) {
        std::thread::sleep(std::time::Duration::from_millis(i));
    }

    #[trace("do_something_async")]
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
root
    do_something_async
    do_something
"#;
    assert_graph(spans, expected_graph);
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
root
    span1
        span4
        span2
            span3
"#;
    assert_graph(spans, expected_graph);
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
root
    span1
        span2
"#;
    assert_graph(spans, expected_graph);
}

#[test]
fn max_span_count() {
    fn block_until_next_collect_loop() {
        let (_, collector) = Span::root("dummy");
        block_on(collector.collect());
    }

    #[trace("recursive")]
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
root
    recursive
        recursive
            recursive
    recursive
        recursive
            recursive
"#;
    assert_graph(spans, expected_graph);
}

fn assert_graph(spans: Vec<SpanRecord>, expected_graph: &str) {
    let result = build_span_graph(spans.clone()).trim().to_string();
    let expected_graph = expected_graph.trim();

    if result != expected_graph {
        panic!(
            "assertion failed: `(result == expected)`\nresult:\n{}\nexpected:\n{}",
            result, expected_graph
        );
    }

    if minstant::is_tsc_available() {
        assert_eq!(spans.iter().filter(|span| span.duration_ns == 0).count(), 0);
    }
}

fn build_span_graph(mut spans: Vec<SpanRecord>) -> String {
    use petgraph::algo::dijkstra;
    use petgraph::prelude::*;
    use std::collections::HashMap;

    spans.sort_by(|a, b| a.event.cmp(b.event));

    let mut span_name: HashMap<u32, &str> = HashMap::new();
    for span in &spans {
        span_name.insert(span.id, span.event);
    }

    let graph: DiGraphMap<u32, ()> =
        DiGraphMap::from_edges(spans.into_iter().map(|span| (span.parent_id, span.id)));

    let mut result = String::new();

    let mut dfs = Dfs::new(&graph, 0);
    // node 0 is not a real span
    dfs.next(&graph).unwrap();
    while let Some(nx) = dfs.next(&graph) {
        let depth = dijkstra(&graph, 0, Some(nx), |_| 1)[&nx] - 1;
        result.push_str(&format!(
            "{:indent$}{}\n",
            "",
            span_name[&nx],
            indent = depth * 4
        ));
    }

    result
}
