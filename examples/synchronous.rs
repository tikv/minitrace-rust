fn func1(i: i32) {
    let span = tracer::new_span();
    let _g = span.enter();

    for j in 0..(i * 10) {
        println!("get {}", j);
    }

    let _ = func2();
}

#[tracer::tracer_attribute::instrument]
fn func2() -> String {
    let mut s = String::new();
    for _ in 0..50 {
        s.push_str(&format!("{:#?}\n", std::time::SystemTime::now()));
    }

    s
}

fn main() {
    let (tx, rx) = crossbeam::channel::unbounded();
    {
        let span = tracer::new_span_root(tx);
        let _g = span.enter();

        for i in 0..10 {
            func1(i);
        }
    }
    tracer::util::draw_stdout(rx.iter().collect());
}
