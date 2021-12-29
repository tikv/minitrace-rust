// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::prelude::*;

fn parallel_job() -> Vec<tokio::task::JoinHandle<()>> {
    let mut v = Vec::with_capacity(4);
    for i in 0..4 {
        v.push(tokio::spawn(
            iter_job(i).in_span(Span::enter_with_local_parent("iter job")),
        ));
    }
    v
}

async fn iter_job(iter: u64) {
    std::thread::sleep(std::time::Duration::from_millis(iter * 10));
    tokio::task::yield_now().await;
    other_job().await;
}

#[trace("other job", enter_on_poll=true)]
async fn other_job() {
    for i in 0..20 {
        if i == 10 {
            tokio::task::yield_now().await;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[tokio::main]
async fn main() {
    let (span, collector) = Span::root("root");

    let f = async {
        let jhs = {
            let _s = LocalSpan::enter_with_local_parent("a span")
                .with_property(|| ("a property", "a value".to_owned()));
            parallel_job()
        };

        other_job().await;

        for jh in jhs {
            jh.await.unwrap();
        }
    }
    .in_span(span);

    tokio::spawn(f).await.unwrap();

    let spans = collector.collect_with_args(CollectArgs::default().sync(true));

    // Report to Jaeger
    let bytes =
        minitrace_jaeger::encode("asynchronous".to_owned(), rand::random(), 0, 0, &spans).unwrap();
    minitrace_jaeger::report("127.0.0.1:6831".parse().unwrap(), &bytes).ok();

    // Report to Datadog
    let bytes = minitrace_datadog::encode("asynchronous", rand::random(), 0, 0, &spans).unwrap();
    minitrace_datadog::report("127.0.0.1:8126".parse().unwrap(), bytes)
        .await
        .ok();
}
