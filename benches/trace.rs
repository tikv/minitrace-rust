use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[derive(Debug, Copy, Clone)]
enum CollectorType {
    Void,
    Bounded,
    Unbounded,
}

fn dummy_iter(i: u16) {
    #[minitrace::trace(0u32)]
    fn dummy() {}

    for _ in 0..i - 1 {
        dummy();
    }
}

#[minitrace::trace(0u32)]
fn dummy_rec(i: u16) {
    if i > 1 {
        dummy_rec(i - 1);
    }
}

fn trace_options() -> Vec<(u16, CollectorType)> {
    let factors = &[1, 10, 100, 1000, 10000];
    let types = &[
        CollectorType::Void,
        CollectorType::Bounded,
        CollectorType::Unbounded,
    ];
    factors
        .iter()
        .flat_map(|factor| types.iter().map(move |tp| (*factor, *tp)))
        .collect()
}

fn build_collect(cap: u16, tp: CollectorType) -> (minitrace::CollectorTx, minitrace::CollectorRx) {
    match tp {
        CollectorType::Void => minitrace::Collector::void(),
        CollectorType::Bounded => minitrace::Collector::bounded(cap),
        CollectorType::Unbounded => minitrace::Collector::unbounded(),
    }
}

fn trace_wide_bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "trace_wide",
        |b, (factor, collect_type)| {
            b.iter(|| {
                let (tx, mut rx) = black_box(build_collect(*factor, *collect_type));
                {
                    let span = minitrace::new_span_root(black_box(tx), black_box(0u32));
                    let _g = black_box(span.enter());

                    if *factor > 1 {
                        dummy_iter(black_box(*factor));
                    }
                }

                let _r = black_box(rx.collect().unwrap());
            });
        },
        trace_options(),
    );
}

fn trace_deep_bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "trace_deep",
        |b, (factor, collect_type)| {
            b.iter(|| {
                let (tx, mut rx) = black_box(build_collect(*factor, *collect_type));

                {
                    let span = minitrace::new_span_root(black_box(tx), black_box(0u32));
                    let _g = black_box(span.enter());

                    if *factor > 1 {
                        dummy_rec(black_box(*factor));
                    }
                }

                let _r = black_box(rx.collect().unwrap());
            });
        },
        trace_options(),
    );
}

fn instant_bench(c: &mut Criterion) {
    c.bench_function("instant", |b| {
        b.iter(minitrace::time::InstantMillis::now);
    });
}

criterion_group!(benches, trace_wide_bench, trace_deep_bench, instant_bench,);
criterion_main!(benches);
