// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

mod common;

#[repr(u32)]
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

fn parallel_job() {
    use minitrace::prelude::*;

    for i in 0..4 {
        tokio::spawn(iter_job(i).trace_task(AsyncJob::IterJob));
    }
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
        std::thread::sleep(std::time::Duration::from_millis(1))
    }
}

#[tokio::main]
async fn main() {
    use minitrace::prelude::*;

    let (root, collector) = minitrace::trace_enable(AsyncJob::Root);

    {
        let _guard = root;
        tokio::spawn(
            async {
                parallel_job();
                other_job().await;
            }
            .trace_task(AsyncJob::Root),
        );
    }

    // waiting for all spans are finished
    std::thread::sleep(std::time::Duration::from_millis(200));

    let r = collector.collect();
    crate::common::draw_stdout(r);
}
