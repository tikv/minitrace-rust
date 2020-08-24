// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn dummy_iter(i: usize) {
    #[minitrace::trace(0u32)]
    fn dummy() {}

    for _ in 0..i - 1 {
        dummy();
    }
}

#[minitrace::trace(0u32)]
fn dummy_rec(i: usize) {
    if i > 1 {
        dummy_rec(i - 1);
    }
}

fn trace_wide_bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "trace_wide",
        |b, len| {
            b.iter(|| {
                let (root, collector) = minitrace::start_trace(0u32);
                {
                    let _guard = root;

                    if *len > 1 {
                        dummy_iter(*len);
                    }
                }

                let _r = black_box(collector.collect());
            });
        },
        vec![1, 10, 100, 1000, 10000],
    );
}

fn trace_deep_bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "trace_deep",
        |b, len| {
            b.iter(|| {
                let (root, collector) = minitrace::start_trace(0u32);

                {
                    let _guard = root;

                    if *len > 1 {
                        dummy_rec(*len);
                    }
                }

                let _r = black_box(collector.collect());
            });
        },
        vec![1, 10, 100, 1000, 10000],
    );
}

criterion_group!(benches, trace_wide_bench, trace_deep_bench);
criterion_main!(benches);
