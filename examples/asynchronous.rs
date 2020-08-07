// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

mod common;
use minitrace::prelude::*;

#[repr(u32)]
#[derive(Debug)]
enum AsyncJob {
    #[allow(dead_code)]
    Unknown = 0u32,
    Root,
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
        v.push(tokio::spawn(iter_job(i).trace_task(AsyncJob::IterJob)));
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
    let (trace_results, _) = async {
        minitrace::property(b"sample property:it works");
        let jhs = parallel_job();
        other_job().await;

        for jh in jhs {
            jh.await.unwrap();
        }
    }
    .future_trace_enable(AsyncJob::Root)
    .await;

    #[cfg(feature = "jaeger")]
    {
        let mut buf = Vec::with_capacity(2048);
        minitrace::jaeger::thrift_encode(&mut buf, "asynchronous_example", &trace_results, |e| {
            format!("{:?}", unsafe { std::mem::transmute::<_, AsyncJob>(e) })
        });

        let agent = std::net::SocketAddr::from(([127, 0, 0, 1], 6831));
        let _ = std::net::UdpSocket::bind(std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
            0,
        ))
        .and_then(move |s| s.send_to(&buf, agent));
    }

    crate::common::draw_stdout(trace_results);
}
