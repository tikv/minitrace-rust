// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use minitrace::LocalCollector;
use minitrace::*;
use minitrace_macro::trace;

fn dummy_iter(i: usize) {
    #[trace("")]
    fn dummy() {}

    for _ in 0..i {
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
    let mut group = c.benchmark_group("trace_wide_raw");

    for len in &[1, 10, 100, 1000, 10000] {
        group.bench_function(len.to_string(), |b| {
            b.iter(|| {
                let local_collector = LocalCollector::start();
                dummy_iter(*len);
                local_collector.collect()
            })
        });
    }

    group.finish();
}

fn trace_wide_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace_wide");

    for len in &[1, 10, 100, 1000, 10000] {
        group.bench_function(len.to_string(), |b| {
            b.iter(|| {
                {
                    let (root_span, collector) = Span::root("root");
                    let _sg = root_span.enter();
                    dummy_iter(*len - 1);
                    collector
                }
                .collect()
            })
        });
    }

    group.finish();
}

fn trace_deep_raw_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace_deep_raw");

    for len in &[1, 10, 100, 1000, 10000] {
        group.bench_function(len.to_string(), |b| {
            b.iter(|| {
                let local_collector = LocalCollector::start();
                dummy_rec(*len);
                local_collector.collect()
            })
        });
    }

    group.finish();
}

fn trace_deep_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace_deep");

    for len in &[1, 10, 100, 1000, 10000] {
        group.bench_function(len.to_string(), |b| {
            b.iter(|| {
                {
                    let (root_span, collector) = Span::root("root");
                    let _sg = root_span.enter();
                    dummy_rec(*len - 1);
                    collector
                }
                .collect()
            })
        });
    }

    group.finish();
}

fn trace_future_bench(c: &mut Criterion) {
    async fn f(i: u32) {
        for _ in 0..i - 1 {
            async {}.in_local_span(black_box("")).await
        }
    }

    let mut group = c.benchmark_group("trace_future");

    for len in &[1, 10, 100, 1000, 10000] {
        group.bench_function(len.to_string(), |b| {
            b.iter(|| {
                {
                    let (root_span, collector) = Span::root("root");
                    let _ = futures::executor::block_on(f(*len).in_span(root_span));
                    collector
                }
                .collect()
            })
        });
    }

    group.finish();
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
