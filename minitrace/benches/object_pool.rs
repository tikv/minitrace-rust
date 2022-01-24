// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use minitrace::util::object_pool::Pool;

fn bench_alloc_vec(c: &mut Criterion) {
    let mut bgroup = c.benchmark_group("Vec::with_capacity(16)");

    for cap in &[1, 10, 100, 1000, 10000, 100000] {
        let vec_pool: Pool<Vec<usize>> = Pool::new(Vec::new, Vec::clear);
        let mut puller = vec_pool.puller(512);
        bgroup.bench_function(format!("object-pool/{}", cap), |b| {
            b.iter_batched(
                || (),
                |_| {
                    let mut vec = puller.pull();
                    if vec.capacity() < *cap {
                        vec.reserve(*cap);
                    }
                    vec
                },
                BatchSize::NumIterations(512),
            )
        });

        bgroup.bench_function(format!("alloc/{}", cap), |b| {
            b.iter_batched(
                || (),
                |_| Vec::<usize>::with_capacity(*cap),
                BatchSize::NumIterations(512),
            )
        });
    }

    bgroup.finish();
}

criterion_group!(benches, bench_alloc_vec);
criterion_main!(benches);
