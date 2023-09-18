// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::Criterion;
use minitrace::local::LocalCollector;
use minitrace::prelude::*;

fn init_minitrace() {
    struct DummyReporter;

    impl minitrace::collector::Reporter for DummyReporter {
        fn report(&mut self, _spans: &[minitrace::prelude::SpanRecord]) {}
    }

    minitrace::set_reporter(DummyReporter, minitrace::collector::Config::default());
}

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
    minitrace::flush();
}

fn bench_trace_wide(c: &mut Criterion) {
    init_minitrace();

    let mut group = c.benchmark_group("trace_wide");

    for len in &[1, 10, 100, 1000, 10000] {
        group.bench_function(len.to_string(), |b| {
            b.iter(|| {
                let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
                let _sg = root.set_local_parent();
                dummy_iter(*len - 1);
            })
        });
    }

    group.finish();
    minitrace::flush()
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
    minitrace::flush()
}

fn bench_trace_deep(c: &mut Criterion) {
    init_minitrace();

    let mut group = c.benchmark_group("trace_deep");

    for len in &[1, 10, 100, 1000] {
        group.bench_function(len.to_string(), |b| {
            b.iter(|| {
                let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
                let _sg = root.set_local_parent();
                dummy_rec(*len - 1);
            })
        });
    }

    group.finish();
    minitrace::flush()
}

fn bench_trace_future(c: &mut Criterion) {
    init_minitrace();

    async fn f(i: u32) {
        for _ in 0..i - 1 {
            async {}.enter_on_poll(black_box("")).await
        }
    }

    let mut group = c.benchmark_group("trace_future");

    for len in &[1, 10, 100, 1000, 10000] {
        group.bench_function(len.to_string(), |b| {
            b.iter(|| {
                let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
                futures::executor::block_on(f(*len).in_span(root));
            })
        });
    }

    group.finish();
    minitrace::flush()
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
