// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::{criterion_group, criterion_main, Criterion};

fn rustracing_harness() {
    fn dummy_rustracing(span: &rustracing::span::Span<()>) {
        for _ in 0..100 {
            let _child_span = span.child("child", |c| c.start_with_state(()));
        }
    }

    let (span_tx, span_rx) = crossbeam::channel::bounded(100);

    {
        let tracer = rustracing::Tracer::with_sender(rustracing::sampler::AllSampler, span_tx);
        let parent_span = tracer.span("parent").start_with_state(());
        dummy_rustracing(&parent_span);
    }

    let _r = span_rx.iter().collect::<Vec<_>>();
}

fn init_opentelemetry() {
    use tracing_subscriber::prelude::*;

    let opentelemetry = tracing_opentelemetry::layer();
    tracing_subscriber::registry()
        .with(opentelemetry)
        .try_init()
        .unwrap();
}

fn opentelemetry_harness() {
    fn dummy_opentelementry() {
        for _ in 0..100 {
            let child = tracing::span!(tracing::Level::TRACE, "child");
            let _enter = child.enter();
        }
    }

    let root = tracing::span!(tracing::Level::TRACE, "parent");
    let _enter = root.enter();

    dummy_opentelementry();
}

fn minitrace_harness() {
    use minitrace::prelude::*;

    fn dummy_minitrace() {
        for _ in 0..100 {
            let _guard = LocalSpan::enter_with_local_parent("child");
        }
    }

    let _spans = {
        let (root_span, collector) = Span::root("parent");
        let _g = root_span.set_local_parent();

        dummy_minitrace();

        collector
    }
    .collect();
}

fn tracing_comparison(c: &mut Criterion) {
    init_opentelemetry();

    let mut bgroup = c.benchmark_group("100 spans");

    bgroup.bench_function("Tokio Tracing", |b| b.iter(opentelemetry_harness));
    bgroup.bench_function("Rustracing", |b| b.iter(rustracing_harness));
    bgroup.bench_function("minitrace", |b| b.iter(minitrace_harness));

    bgroup.finish();
}

criterion_group!(benches, tracing_comparison);
criterion_main!(benches);
