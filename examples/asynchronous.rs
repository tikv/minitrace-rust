use tracer::future::Instrument;

#[tracer::tracer_attribute::instrument]
async fn parallel_job() {
    for i in 0..4 {
        tokio::spawn(iter_job(i).in_current_span("parallel_iter_job"));
    }
}

async fn iter_job(_iter: i32) {
    for _ in 0..20 {
        println!("b");
    }
    tokio::task::yield_now().await;
    other_job().await;
}

#[tracer::tracer_attribute::instrument("other_job 💯")]
async fn other_job() {
    for i in 0..20 {
        if i == 10 {
            tokio::task::yield_now().await;
        }
        println!("a");
    }
}

#[tokio::main]
async fn main() {
    let (tx, rx) = crossbeam::channel::unbounded();

    tokio::spawn(
        async {
            parallel_job().await;
            other_job().await;
        }
        .instrument(tracer::new_span_root("root", tx)),
    );

    tracer::util::draw_stdout(rx.iter().collect());
}
