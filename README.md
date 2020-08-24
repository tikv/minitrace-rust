# Minitrace
[![Actions Status](https://github.com/pingcap-incubator/minitrace/workflows/CI/badge.svg)](https://github.com/pingcap-incubator/minitrace/actions)
[![LICENSE](https://img.shields.io/github/license/pingcap-incubator/minitrace.svg)](https://github.com/pingcap-incubator/minitrace/blob/master/LICENSE)

A high-performance, ergonomic timeline tracing library for Rust.


## Usage

```toml
[dependencies]
minitrace = { git = "https://github.com/pingcap-incubator/minitrace-rust.git" }
```

### In Synchronous Code

```rust
let (root_guard, collector) = minitrace::start_trace(0u32);
minitrace::new_property(b"tracing started");

{
    let _child_guard = minitrace::new_span(1u32);
    minitrace::new_property(b"in child");
}

drop(root_guard);
let trace_results = collector.unwrap().collect();
```

### In Asynchronous Code

Futures:

```rust
use minitrace::future::FutureExt as _;

let task = async {
    let guard = minitrace::new_span(1u32);
    // current future ...

    // should drop here or it would fail compilation
    // because local guards cannot across threads.
    drop(guard);

    async {
        // current future ...
    }.in_new_span(2u32).await;

    runtime::spawn(async {
        // new future ...
        minitrace::new_property(b"spawned to some runtime");
    }.in_new_span(3u32));

    async {
        // current future ...
    }.in_new_span(4u32).await;
};

let (collector, value) = runtime::block_on(task.collect_trace(0u32));
let trace_results = collector.unwrap().collect();
```

Threads:

```rust
let (root, collector) = minitrace::start_trace(0u32);

let mut handle = minitrace::thread::new_async_scope();

let th = std::thread::spawn(move || {
    let _parent_guard = handle.start_trace(1u32);

    {
        let _child_guard = minitrace::new_span(2u32);
    }
});

drop(root);

th.join().unwrap();
let trace_results = collector.unwrap().collect();
```


## Timeline Examples


### Setup JaegerUI
```sh
$ docker run --rm -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 jaegertracing/all-in-one:latest
```

### Run Synchronous Example
```sh
$ cargo run --features "jaeger" --example synchronous
====================================================================== 111.69 ms
=                                                                        2.13 ms
                                                                         1.06 ms
 ==                                                                      4.14 ms
  =                                                                      2.07 ms
   ===                                                                   6.16 ms
     =                                                                   3.08 ms
       =====                                                             8.18 ms
          ==                                                             4.09 ms
            ======                                                      10.20 ms
                ===                                                      5.10 ms
                   =======                                              12.18 ms
                       ===                                               6.09 ms
                          ========                                      14.15 ms
                               ====                                      7.08 ms
                                   ==========                           16.16 ms
                                        =====                            8.08 ms
                                             ===========                18.17 ms
                                                   =====                 9.08 ms
                                                         ============   20.17 ms
                                                               ======   10.08 ms
```
![Jaeger Synchronous](img/jaeger-synchronous.png)

### Run Asynchronous Example
```sh
$ cargo run --features "jaeger" --example asynchronous
============================                                            21.81 ms
==============                                                          10.84 ms
============================                                            21.67 ms
==============                                                          10.84 ms
              ==============                                            10.77 ms
============= ============================                              31.50 ms
              ==============                                            10.70 ms
                            ==============                              10.65 ms
========================================= ==============                41.52 ms
                           ==============                               10.72 ms
                                          ==============                10.63 ms
======================================== ============================   51.34 ms
                                         ==============                 10.60 ms
                                                       ==============   10.61 ms
              ==============                                            10.74 ms
```
![Jaeger Asynchronous](img/jaeger-asynchronous.png)
