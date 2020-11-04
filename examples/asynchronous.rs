// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::future::FutureExt;
use minitrace::*;
use minitrace_jaeger::Reporter;
use minitrace_macro::trace_async;
use std::net::{Ipv4Addr, SocketAddr};

fn parallel_job() -> Vec<tokio::task::JoinHandle<()>> {
    let mut v = Vec::with_capacity(4);
    for i in 0..4 {
        v.push(tokio::spawn(iter_job(i).in_new_scope("iter job")));
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
    let (scope, collector) = root_scope("root");

    let f = async {
        let jhs = {
            let _s = new_span("a span").with_property(|| ("a property", "a value".to_owned()));
            parallel_job()
        };

        other_job().await;

        for jh in jhs {
            jh.await.unwrap();
        }
    }
    .with_scope(scope);

    tokio::spawn(f).await.unwrap();

    let spans = collector.collect(true, None, None);
    let socket = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 6831);
    let reporter = Reporter::new(socket, "asynchronous");
    reporter.report(rand::random(), spans).ok();
}
