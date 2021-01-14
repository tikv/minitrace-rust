// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use minitrace::LocalCollector;
use minitrace::*;
use minitrace_macro::trace;

fn dummy_iter(i: usize) {
    #[trace("")]
    fn dummy() {}

    for _ in 0..i - 1 {
        dummy();
    }
}

#[trace("")]
fn dummy_rec(i: usize) {
    if i > 1 {
        dummy_rec(i - 1);
    }
}

fn trace_wide_raw_bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "trace_wide_raw",
        |b, len| {
            b.iter(|| {
                let local_collector = LocalCollector::start();
                dummy_iter(*len);
                local_collector.collect()
            });
        },
        vec![1, 10, 100, 1000, 10000],
    );
}

fn trace_wide_bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "trace_wide",
        |b, len| {
            b.iter(|| {
                {
                    let (root_scope, collector) = Scope::root("root");

                    let _sg = root_scope.enter();

                    if *len > 1 {
                        dummy_iter(*len);
                    }

                    collector
                }
                .collect()
            });
        },
        vec![1, 10, 100, 1000, 10000],
    );
}

fn trace_deep_raw_bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "trace_deep_raw",
        |b, len| {
            b.iter(|| {
                let local_collector = LocalCollector::start();
                dummy_rec(*len);
                local_collector.collect()
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
                {
                    let (root_scope, collector) = Scope::root("root");

                    let _sg = root_scope.enter();

                    if *len > 1 {
                        dummy_rec(*len);
                    }

                    collector
                }
                .collect()
            });
        },
        vec![1, 10, 100, 1000, 10000],
    );
}

fn trace_future_bench(c: &mut Criterion) {
    async fn f(i: u32) {
        for _ in 0..i - 1 {
            async {}.in_new_span(black_box("")).await
        }
    }

    c.bench_function_over_inputs(
        "trace_future",
        |b, len| {
            b.iter(|| {
                {
                    let (root_scope, collector) = Scope::root("root");

                    let _ = futures::executor::block_on(f(*len).with_scope(root_scope));

                    collector
                }
                .collect()
            });
        },
        vec![1, 10, 100, 1000, 10000],
    );
}

criterion_group!(
    benches,
    trace_wide_raw_bench,
    trace_wide_bench,
    trace_deep_raw_bench,
    trace_deep_bench,
    trace_future_bench
);
criterion_main!(benches);
