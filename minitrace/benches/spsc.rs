// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::{criterion_group, criterion_main, Criterion};

fn crossbeam(nmsg: usize) {
    let (tx, rx) = crossbeam::channel::unbounded();

    crossbeam::scope(|scope| {
        scope.spawn(|_| {
            for i in 0..nmsg {
                tx.send(i).unwrap();
            }
        });

        for _ in 0..nmsg {
            while let Ok(_) = rx.try_recv() {}
        }
    })
    .unwrap();
}

fn crossbeam_send_only(nmsg: usize) {
    let (tx, _rx) = crossbeam::channel::unbounded();

    for i in 0..nmsg {
        tx.send(i).unwrap();
    }
}

fn minitrace(nmsg: usize) {
    let (tx, mut rx) = minitrace::util::spsc::unbounded();

    crossbeam::scope(|scope| {
        scope.spawn(|_| {
            for i in 0..nmsg {
                tx.send(i);
            }
        });

        for _ in 0..nmsg {
            while let Some(_) = rx.try_recv().unwrap() {}
        }
    })
    .unwrap();
}

fn minitrace_send_only(nmsg: usize) {
    let (tx, _rx) = minitrace::util::spsc::unbounded();

    for i in 0..nmsg {
        tx.send(i);
    }
}

fn spsc_comparison(c: &mut Criterion) {
    let mut bgroup = c.benchmark_group("spsc channel");

    for len in &[1, 10, 100, 1000, 10000] {
        bgroup.bench_function(format!("crossbeam-{}", len), |b| b.iter(|| crossbeam(*len)));
        bgroup.bench_function(format!("minitrace-{}", len), |b| b.iter(|| minitrace(*len)));
    }

    bgroup.finish();
}

fn spsc_send_only_comparison(c: &mut Criterion) {
    let mut bgroup = c.benchmark_group("spsc channel send only");

    for len in &[1, 10, 100, 1000, 10000] {
        bgroup.bench_function(format!("crossbeam-{}", len), |b| {
            b.iter(|| crossbeam_send_only(*len))
        });
        bgroup.bench_function(format!("minitrace-{}", len), |b| {
            b.iter(|| minitrace_send_only(*len))
        });
    }

    bgroup.finish();
}

criterion_group!(benches, spsc_comparison, spsc_send_only_comparison);
criterion_main!(benches);
