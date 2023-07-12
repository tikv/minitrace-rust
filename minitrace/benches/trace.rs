// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use minitrace::local::LocalCollector;
use minitrace::prelude::*;

fn dummy_iter(i: usize) {
    #[trace]
    fn dummy() {}

    for _ in 0..i {
        dummy();
    }
}

#[trace]
fn dummy_rec(i: usize) {
    if i > 1 {
        dummy_rec(i - 1);
    }
}

fn bench_trace_wide_raw(c: &mut Criterion) {
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

fn bench_trace_wide(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace_wide");

    for len in &[1, 10, 100, 1000, 10000] {
        group.bench_function(format!("with-collect-{}", len), |b| {
            b.iter(|| {
                {
                    let (root_span, collector) = Span::root("root");
                    let _sg = root_span.set_local_parent();
                    dummy_iter(*len - 1);
                    collector
                }
                .collect()
            })
        });
        group.bench_function(format!("without-collect-{}", len), |b| {
            b.iter(|| {
                let (root_span, _) = Span::root("root");
                let _sg = root_span.set_local_parent();
                dummy_iter(*len - 1);
            })
        });
    }

    group.finish();
}

fn bench_trace_deep_raw(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace_deep_raw");

    for len in &[1, 10, 100, 1000] {
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

fn bench_trace_deep(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace_deep");

    for len in &[1, 10, 100, 1000] {
        group.bench_function(format!("with-collect-{}", len), |b| {
            b.iter(|| {
                {
                    let (root_span, collector) = Span::root("root");
                    let _sg = root_span.set_local_parent();
                    dummy_rec(*len - 1);
                    collector
                }
                .collect()
            })
        });
        group.bench_function(format!("without-collect-{}", len), |b| {
            b.iter(|| {
                let (root_span, _) = Span::root("root");
                let _sg = root_span.set_local_parent();
                dummy_rec(*len - 1);
            })
        });
    }

    group.finish();
}

fn bench_trace_future(c: &mut Criterion) {
    async fn f(i: u32) {
        for _ in 0..i - 1 {
            async {}.enter_on_poll(black_box("")).await
        }
    }

    let mut group = c.benchmark_group("trace_future");

    for len in &[1, 10, 100, 1000, 10000] {
        group.bench_function(len.to_string(), |b| {
            b.iter(|| {
                {
                    let (root_span, collector) = Span::root("root");
                    futures::executor::block_on(f(*len).in_span(root_span));
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
    bench_trace_wide_raw,
    bench_trace_wide,
    bench_trace_deep_raw,
    bench_trace_deep,
    bench_trace_future
);
criterion_main!(benches);
