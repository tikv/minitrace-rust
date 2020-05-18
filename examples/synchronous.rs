fn func1(i: u64) {
    minitrace::block!(0u32, {
        for j in 0..(i * 10) {
            std::thread::sleep(std::time::Duration::from_micros(j));
        }
    });

    func2();
}

#[minitrace::trace(0u32)]
fn func2() {
    for i in 0..50 {
        std::thread::sleep(std::time::Duration::from_micros(i));
    }
}

fn main() {
    let (tx, mut rx) = minitrace::Collector::bounded(256);
    {
        let span = minitrace::new_span_root(tx, 0u32);
        let _enter = span.enter();
        for i in 0..10 {
            func1(i);
        }
    }
    minitrace::util::draw_stdout(rx.collect().unwrap());
}
