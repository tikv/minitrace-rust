fn func1(i: i32) {
    let span = tracer::new_span("func1");
    let _g = span.enter();

    for j in 0..(i * 10) {
        println!("get {}", j);
    }

    let _ = func2();
}

#[tracer::tracer_attribute::instrument("func2 😻")]
fn func2() -> String {
    let mut s = String::new();
    for _ in 0..50 {
        s.push_str(&format!("{:#?}\n", std::time::SystemTime::now()));
    }

    s
}

fn main() {
    let tracer::Collector { tx, rx } = tracer::Collector::new(tracer::COLLECTOR_TYPE);
    {
        let span = tracer::new_span_root("root", tx, tracer::TIME_MEASURE_TYPE);
        let _g = span.enter();
        for i in 0..10 {
            func1(i);
        }
    }
    tracer::util::draw_stdout(rx.collect_all());
}
