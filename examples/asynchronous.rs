use tracer::future::Instrument;

#[repr(u32)]
enum AsyncJob {
    #[allow(dead_code)]
    Unknown = 0u32,
    Root,
    ParallelJob,
    IterJob,
    OtherJob,
}

#[tracer::trace(AsyncJob::ParallelJob)]
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

#[tracer::trace(AsyncJob::OtherJob)]
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
    let (tx, rx) = tracer::Collector::new(tracer::COLLECTOR_TYPE);

    tokio::spawn(
        async {
            parallel_job().await;
            other_job().await;
        }
        .instrument(tracer::new_span_root(tx, AsyncJob::Root as u32)),
    );

    tracer::util::draw_stdout(rx.collect_all());
}
