# Minitrace
[![Actions Status](https://github.com/pingcap-incubator/minitrace/workflows/CI/badge.svg)](https://github.com/pingcap-incubator/minitrace/actions)
[![LICENSE](https://img.shields.io/github/license/pingcap-incubator/minitrace.svg)](https://github.com/pingcap-incubator/minitrace/blob/master/LICENSE)

A high-performance, ergonomic timeline tracing library for Rust.


## Usage

```toml
[dependencies]
minitrace = { git = "https://github.com/pingcap-incubator/minitrace.git" }
```

### In Synchronous Code

```rust
let (root, collector) = minitrace::trace_enable(0);
{
    let _parent_guard = root;
    {
        let _child_guard = minitrace::new_span(1);  
    }
}

let spans = collector.collect();
```

### In Asynchronous Code

Futures:

```rust
use minitrace::prelude::*;

let task = async {
    let guard = minitrace::new_span(1);
    // ...
    drop(guard);

    async {}.trace_async(2).await;

    runtime::spawn(async {}.trace_task(3));

    async {}.trace_async(4).await;
};

let (spans, _r) = runtime::block_on(task.trace_root_future(0));
```

Threads:

```rust
let (root, collector) = minitrace::trace_enable(0);

let handle = minitrace::trace_crossthread(1);

std::thread::spawn(move || {
    let mut handle = handle;
    let _parent_guard = handle.trace_enable();

    {
        let _child_guard = minitrace::new_span(2);
    }
});

drop(root);

let spans = collector.collect();
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
