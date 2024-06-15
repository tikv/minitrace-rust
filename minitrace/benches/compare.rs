// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;
use tracing::span;
use tracing::Event;
use tracing::Id;
use tracing::Metadata;
use tracing_core::span::Current;

/// A collector that is enabled but otherwise does nothing.
struct EnabledSubscriber;

impl tracing::Subscriber for EnabledSubscriber {
    fn new_span(&self, span: &span::Attributes<'_>) -> Id {
        let _ = span;
        Id::from_u64(0xDEAD_FACE)
    }

    fn event(&self, event: &Event<'_>) {
        let _ = event;
    }

    fn record(&self, span: &Id, values: &span::Record<'_>) {
        let _ = (span, values);
    }

    fn record_follows_from(&self, span: &Id, follows: &Id) {
        let _ = (span, follows);
    }

    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        let _ = metadata;
        true
    }

    fn enter(&self, span: &Id) {
        let _ = span;
    }

    fn exit(&self, span: &Id) {
        let _ = span;
    }

    fn current_span(&self) -> Current {
        Current::none()
    }
}

fn init_minitrace() {
    struct DummyReporter;

    impl minitrace::collector::Reporter for DummyReporter {
        fn report(&mut self, _spans: &[minitrace::prelude::SpanRecord]) {}
    }

    minitrace::set_reporter(DummyReporter, minitrace::collector::Config::default());
}

fn tracing_harness(n: usize) {
    fn dummy_span(n: usize) {
        for _ in 0..n {
            let child = tracing::span!(tracing::Level::TRACE, "child");
            let _enter = child.enter();
        }
    }

    let root = tracing::span!(tracing::Level::TRACE, "parent");
    let _enter = root.enter();

    dummy_span(n);
}

fn rustracing_harness(n: usize) {
    fn dummy_rustracing(n: usize, span: &rustracing::span::Span<()>) {
        for _ in 0..n {
            let _child_span = span.child("child", |c| c.start_with_state(()));
        }
    }

    let (span_tx, span_rx) = crossbeam::channel::bounded(1000);

    {
        let tracer = rustracing::Tracer::with_sender(rustracing::sampler::AllSampler, span_tx);
        let parent_span = tracer.span("parent").start_with_state(());
        dummy_rustracing(n, &parent_span);
    }

    let _r = span_rx.iter().collect::<Vec<_>>();
}

fn minitrace_harness(n: usize) {
    use minitrace::prelude::*;

    fn dummy_minitrace(n: usize) {
        for _ in 0..n {
            let _guard = LocalSpan::enter_with_local_parent("child");
        }
    }

    let root = Span::root("parent", SpanContext::new(TraceId(12), SpanId::default()));
    let _g = root.set_local_parent();

    dummy_minitrace(n);
}

fn tracing_comparison(c: &mut Criterion) {
    use tracing_subscriber::prelude::*;

    let mut group = c.benchmark_group("Comparison");

    for n in &[1, 10, 100, 1000] {
        init_minitrace();
        group.bench_function(BenchmarkId::new("minitrace-noop", n), |b| {
            b.iter(|| minitrace_harness(*n))
        });

        let subscriber = EnabledSubscriber;
        tracing::subscriber::with_default(subscriber, || {
            group.bench_with_input(BenchmarkId::new("tokio/tracing-noop", n), n, |b, n| {
                b.iter(|| tracing_harness(*n))
            });
        });

        let subscriber = tracing_subscriber::registry().with(tracing_opentelemetry::layer());
        tracing::subscriber::with_default(subscriber, || {
            group.bench_with_input(BenchmarkId::new("tokio/tracing-otel", n), n, |b, n| {
                b.iter(|| tracing_harness(*n))
            });
        });

        group.bench_function(BenchmarkId::new("rusttracing", n), |b| {
            b.iter(|| rustracing_harness(*n))
        });
    }

    group.finish();
}

criterion_group!(benches, tracing_comparison);
criterion_main!(benches);
