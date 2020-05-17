use minitrace::future::Instrument;

#[repr(u32)]
enum AsyncJob {
    #[allow(dead_code)]
    Unknown = 0u32,
    Root,
    ParallelJob,
    IterJob,
    OtherJob,
}

#[minitrace::trace_async(AsyncJob::ParallelJob)]
async fn parallel_job() {
    for i in 0..4 {
        tokio::spawn(iter_job(i).in_current_span(AsyncJob::IterJob as u32));
    }
}

async fn iter_job(_iter: i32) {
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(1))
    }
    tokio::task::yield_now().await;
    other_job().await;
}

#[minitrace::trace_async_fine(AsyncJob::OtherJob)]
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
    let (tx, mut rx) = minitrace::Collector::bounded(256);

    {
        tokio::spawn(
            async {
                parallel_job().await;
                other_job().await;
            }
            .instrument(minitrace::new_span_root(tx, AsyncJob::Root as u32)),
        );
    }

    // waiting for all spans are finished
    std::thread::sleep(std::time::Duration::from_secs(1));

    let r = rx.collect().unwrap();
    minitrace::util::draw_stdout(r);
}
