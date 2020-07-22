// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

mod common;

fn func1(i: u64) {
    let _guard = minitrace::new_span(0u32);
    for _ in 0..i * 1000 {
        std::process::id();
    }
    func2(i);
}

#[minitrace::trace(0u32)]
fn func2(i: u64) {
    for _ in 0..i * 1000 {
        std::process::id();
    }
}

fn main() {
    let (root, collector) = minitrace::trace_enable(0u32);
    {
        let _guard = root;
        for i in 1..=10 {
            func1(i);
        }
    }

    crate::common::draw_stdout(collector.collect());
}
