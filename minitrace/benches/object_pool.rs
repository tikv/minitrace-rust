// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;

use criterion::{criterion_group, criterion_main, Criterion};
use minitrace::util::object_pool::{Pool, Puller, Reusable};
use once_cell::sync::Lazy;

static VEC_POOL: Lazy<Pool<Vec<usize>>> = Lazy::new(|| Pool::new(Vec::new, Vec::clear));

thread_local! {
    static VEC_PULLER: std::cell::RefCell<Puller<'static, Vec<usize>>> = RefCell::new(VEC_POOL.puller(512));
}

type VECS = Reusable<'static, Vec<usize>>;

fn alloc_vec() -> VECS {
    VEC_PULLER.with(|puller| puller.borrow_mut().pull())
}

fn bench_alloc_vec(c: &mut Criterion) {
    let mut bgroup = c.benchmark_group("Vec::with_capacity(16)");

    bgroup.bench_function("alloc", |b| {
        b.iter_with_large_drop(|| Vec::<usize>::with_capacity(16))
    });
    bgroup.bench_function("object-pool", |b| {
        b.iter_with_large_drop(|| {
            let mut vec = alloc_vec();
            if vec.capacity() < 16 {
                vec.reserve(16);
            }
            vec
        })
    });

    bgroup.finish();
}

criterion_group!(benches, bench_alloc_vec);
criterion_main!(benches);
