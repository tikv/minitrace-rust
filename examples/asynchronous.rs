use tracer::future::Instrument;

#[tracer_attribute::instrument]
async fn parallel() {
    for i in 0..4 {
        tokio::spawn(iter_work(i).in_current_span());
    }
}

async fn iter_work(_iter: i32) {
    for _ in 0..20 {
        println!("b");
    }
    tokio::task::yield_now().await;
    other_work().await;
}

#[tracer_attribute::instrument]
async fn other_work() {
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

    tokio::spawn(async {
        parallel().await;
        other_work().await;
    }.instrument(tracer::new_span_root(tx)));

    tracer::util::draw_stdout(rx.iter().collect());
}
