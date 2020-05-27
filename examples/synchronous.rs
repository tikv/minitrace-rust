fn func1(i: u64) {
    let _guard = minitrace::new_span(0);
    std::thread::sleep(std::time::Duration::from_millis(i));
    func2(i);
}

#[minitrace::trace(0u32)]
fn func2(i: u64) {
    std::thread::sleep(std::time::Duration::from_millis(i));
}

fn main() {
    let (root, collector) = minitrace::trace_enable(0);
    {
        let _guard = root;
        for i in 1..=10 {
            func1(i);
        }
    }

    minitrace::util::draw_stdout(collector.collect());
}
