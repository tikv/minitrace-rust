fn func1(i: u64) {
    let span = tracer::new_span(0u32);
    let _g = span.enter();

    for j in 0..(i * 10) {
        std::thread::sleep(std::time::Duration::from_micros(j));
    }

    func2();
}

#[tracer::trace(0u32)]
fn func2() {
    for i in 0..50 {
        std::thread::sleep(std::time::Duration::from_micros(i));
    }
}

fn main() {
    let (tx, rx) = tracer::Collector::new(tracer::COLLECTOR_TYPE);
    {
        let span = tracer::new_span_root(tx, 0u32);
        let _g = span.enter();
        for i in 0..10 {
            func1(i);
        }
    }
    tracer::util::draw_stdout(rx.collect_all());
}
