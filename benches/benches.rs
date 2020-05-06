use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn root_span_new_drop_bench(c: &mut Criterion) {
    c.bench_function("span_root channel instant", |b| {
        let tracer::Collector { tx, _rx } = tracer::Collector::new(tracer::COLLECTOR_TYPE);

        b.iter(|| {
            tracer::new_span_root(
                black_box("root"),
                black_box(tx.clone()),
                black_box(tracer::TIME_MEASURE_TYPE),
            )
        });
    });
}

criterion_group!(
    benches,
    root_span_new_drop_bench,
);
criterion_main!(benches);
