use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn dummy_iter(i: u32) {
    #[minitrace::trace(0u32)]
    fn dummy() {}

    for _ in 0..i {
        dummy();
    }
}

#[minitrace::trace(0u32)]
fn dummy_rec(i: u32) {
    if i == 1 {
        return;
    } else {
        dummy_rec(i - 1);
    }
}

fn trace_wide_bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "trace_wide",
        |b, (factor, to_collect)| {
            b.iter(|| {
                let (tx, rx) = black_box(minitrace::Collector::new(black_box(if *to_collect {
                    black_box(minitrace::CollectorType::Channel)
                } else {
                    black_box(minitrace::CollectorType::Void)
                })));

                {
                    let span = minitrace::new_span_root(black_box(tx), black_box(0u32));
                    let _g = black_box(span.enter());

                    if *factor > 1 {
                        dummy_iter(black_box(*factor));
                    }
                }

                let _r = black_box(rx.collect());
            });
        },
        &[
            (1, false),
            (1, true),
            (10, false),
            (10, true),
            (100, false),
            (100, true),
            (1000, false),
            (1000, true),
        ],
    );
}

fn trace_deep_bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "trace_deep",
        |b, (factor, to_collect)| {
            b.iter(|| {
                let (tx, rx) = black_box(minitrace::Collector::new(black_box(if *to_collect {
                    black_box(minitrace::CollectorType::Channel)
                } else {
                    black_box(minitrace::CollectorType::Void)
                })));

                {
                    let span = minitrace::new_span_root(black_box(tx), black_box(0u32));
                    let _g = black_box(span.enter());

                    if *factor > 1 {
                        dummy_rec(black_box(*factor));
                    }
                }

                let _r = black_box(rx.collect());
            });
        },
        &[
            (1, false),
            (1, true),
            (10, false),
            (10, true),
            (100, false),
            (100, true),
            (1000, false),
            (1000, true),
        ],
    );
}

fn instant_bench(c: &mut Criterion) {
    c.bench_function("instant", |b| {
        b.iter(|| minitrace::time::InstantMillis::now());
    });
}

criterion_group!(benches, trace_wide_bench, trace_deep_bench, instant_bench,);
criterion_main!(benches);
