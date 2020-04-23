fn func1(i: i32) {
    let span = tracer::new_span();
    let _g = span.enter();

    for j in 0..(i * 10) {
        println!("get {}", j);
    }
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
