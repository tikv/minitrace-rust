// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::*;
use minitrace_datadog::Reporter as DReporter;
use minitrace_jaeger::Reporter as JReporter;
use minitrace_macro::trace_async;

fn parallel_job() -> Vec<tokio::task::JoinHandle<()>> {
    let mut v = Vec::with_capacity(4);
    for i in 0..4 {
        v.push(tokio::spawn(
            iter_job(i).with_scope(Scope::from_local_parent("iter job")),
        ));
    }
    v
}

async fn iter_job(iter: u64) {
    std::thread::sleep(std::time::Duration::from_millis(iter * 10));
    tokio::task::yield_now().await;
    other_job().await;
}

#[trace_async("other job")]
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
    let (scope, collector) = Scope::root("root");

    let f = async {
        let jhs = {
            let _s = Span::enter("a span").with_property(|| ("a property", "a value".to_owned()));
            parallel_job()
        };

        other_job().await;

        for jh in jhs {
            jh.await.unwrap();
        }
    }
    .with_scope(scope);

    tokio::spawn(f).await.unwrap();

    let spans = collector.collect_with_args(CollectArgs::default().sync(true));

    // Report to Jaeger
    let bytes = JReporter::encode("asynchronous".to_owned(), rand::random(), 0, 0, &spans).unwrap();
    JReporter::report("127.0.0.1:6831".parse().unwrap(), &bytes).ok();

    // Report to Datadog
    let bytes = DReporter::encode("asynchronous", rand::random(), 0, 0, &spans).unwrap();
    DReporter::report("127.0.0.1:8126".parse().unwrap(), bytes)
        .await
        .ok();
}
