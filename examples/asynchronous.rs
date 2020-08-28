// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

mod common;

use minitrace::future::FutureExt as _;

#[derive(Debug)]
enum AsyncJob {
    #[allow(dead_code)]
    Unknown,
    Root,
    Loop,
    IterJob,
    OtherJob,
}

impl Into<u32> for AsyncJob {
    fn into(self) -> u32 {
        self as u32
    }
}

fn parallel_job() -> Vec<tokio::task::JoinHandle<()>> {
    let mut v = Vec::with_capacity(4);
    for i in 0..4 {
        v.push(tokio::spawn(iter_job(i).in_new_span(AsyncJob::IterJob)));
    }
    v
}

async fn iter_job(iter: u64) {
    std::thread::sleep(std::time::Duration::from_millis(iter * 10));
    tokio::task::yield_now().await;
    other_job().await;
}

#[minitrace::trace_async(AsyncJob::OtherJob)]
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
    let (root, tracker) = minitrace::start_trace(AsyncJob::Root).unwrap();

    let _ = async {
        minitrace::new_property(b"sample property:it works");
        let jhs = parallel_job();
        other_job().await;

        for jh in jhs {
            jh.await.unwrap();
        }
    }
    .in_new_span(AsyncJob::Loop)
    .await;

    drop(root);

    let trace_result = tracker.finish().collect();

    use std::net::SocketAddr;
    let mut buf = Vec::with_capacity(2048);
    minitrace_jaeger::thrift_compact_encode(
        &mut buf,
        "Async Example",
        rand::random(),
        rand::random(),
        &trace_result,
        |e| {
            format!("{:?}", unsafe {
                std::mem::transmute::<_, AsyncJob>(e as u8)
            })
        },
        |property| {
            let mut split = property.splitn(2, |b| *b == b':');
            let key = String::from_utf8_lossy(split.next().unwrap()).to_owned();
            let value = String::from_utf8_lossy(split.next().unwrap()).to_owned();
            (key, value)
        },
    );

    let local_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
    if let Ok(mut socket) = tokio::net::UdpSocket::bind(local_addr).await {
        let _ = socket.send_to(&buf, "127.0.0.1:6831").await;
    }

    crate::common::draw_stdout(trace_result);
}
