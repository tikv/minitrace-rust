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
                let _root = minitrace::start_trace(0u32);

                if *len > 1 {
                    dummy_iter(*len);
                }
            });

            minitrace::collect_all();
        },
        vec![1, 10, 100, 1000, 10000],
    );
}

fn trace_deep_bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "trace_deep",
        |b, len| {
            b.iter(|| {
                let _root = minitrace::start_trace(0u32);

                if *len > 1 {
                    dummy_rec(*len);
                }
            });

            minitrace::collect_all();
        },
        vec![1, 10, 100, 1000, 10000],
    );
}

fn bench_collect(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "bench_collect",
        |b, len| {
            {
                let _root = minitrace::start_trace(0u32);
                
                if *len > 1 {
                    dummy_rec(*len);
                }
            }

            b.iter(|| {
                black_box(minitrace::collect_all())
            });
        },
        vec![1, 10, 100, 1000, 10000],
    );
}


fn bench_new_async_span(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "bench_new_async_span",
        |b, len| {
            let _root = minitrace::start_trace(0u32);

            b.iter(|| {
                for _ in 0..*len {
                    let _guard = black_box(minitrace::new_async_span());
                }
            });

            minitrace::collect_all();
        },
        vec![1, 10, 100, 1000, 10000],
    );
}

criterion_group!(benches, trace_wide_bench, trace_deep_bench, bench_collect, bench_new_async_span);
criterion_main!(benches);
