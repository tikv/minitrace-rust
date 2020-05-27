use minitrace::prelude::*;

#[repr(u32)]
enum AsyncJob {
    #[allow(dead_code)]
    Unknown = 0u32,
    Root,
    IterJob,
    OtherJob,
}

fn parallel_job() {
    for i in 0..4 {
        tokio::spawn(iter_job(i).trace_task(AsyncJob::IterJob as u32));
    }
}

async fn iter_job(iter: u64) {
    std::thread::sleep(std::time::Duration::from_millis(iter * 10));
    tokio::task::yield_now().await;
    other_job().await;
}

#[minitrace::trace_async(AsyncJob::OtherJob as u32)]
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
    let (root, collector) = minitrace::trace_enable(AsyncJob::Root as u32);

    {
        let _guard = root;
        tokio::spawn(
            async {
                parallel_job();
                other_job().await;
            }
            .trace_task(AsyncJob::Root as u32),
        );
    }

    // waiting for all spans are finished
    std::thread::sleep(std::time::Duration::from_secs(1));

    let r = collector.collect();
    minitrace::util::draw_stdout(r);
}
