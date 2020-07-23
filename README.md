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
let (root_guard, collector) = minitrace::trace_enable(0u32);
minitrace::property(b"tracing started");

{
    let _child_guard = minitrace::new_span(1u32);
    minitrace::property(b"in child");
}

drop(root_guard);
let trace_details = collector.collect();
```

### In Asynchronous Code

Futures:

```rust
use minitrace::prelude::*;

let task = async {
    let guard = minitrace::new_span(1u32);
    // current future ...

    // should drop here or it would fail compilation
    // because local guards cannot across threads.
    drop(guard);

    async {
        // current future ...
    }.trace_async(2u32).await;

    runtime::spawn(async {
        // new future ...
        minitrace::property(b"spawned to some runtime");
    }.trace_task(3u32));

    async {
        // current future ...
    }.trace_async(4u32).await;
};

let (trace_details, value) = runtime::block_on(task.future_trace_enable(0u32));
```

Threads:

```rust
let (root, collector) = minitrace::trace_enable(0u32);

let handle = minitrace::trace_crossthread();

let th = std::thread::spawn(move || {
    let mut handle = handle;
    let _parent_guard = handle.trace_enable(1u32);

    {
        let _child_guard = minitrace::new_span(2u32);
    }
});

drop(root);

th.join().unwrap();
let trace_details = collector.collect();
```


## Timeline Examples

```sh
$ cargo +nightly run --example synchronous
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

```sh
$ cargo +nightly run --example asynchronous
=============             =============                                 21.49 ms
=============                                                           10.65 ms
                          ==========================                    21.33 ms
                          =============                                 10.66 ms
                                       =============                    10.64 ms
=============             ==========================                    31.62 ms
                          =============                                 10.66 ms
                                       =============                    10.63 ms
========================= ============= =============                   41.30 ms
                          =============                                 10.62 ms
                                        =============                   10.57 ms
   ====================================== ==========================    51.43 ms
                                          =============                 10.57 ms
                                                       =============    10.79 ms
                          =============                                 10.71 ms
```
