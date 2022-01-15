# minitrace

[![Actions Status](https://github.com/tikv/minitrace-rust/workflows/CI/badge.svg)](https://github.com/tikv/minitrace-rust/actions)
[![Documentation](https://docs.rs/minitrace/badge.svg)](https://docs.rs/minitrace/)
[![Crates.io](https://img.shields.io/crates/v/minitrace.svg)](https://crates.io/crates/minitrace)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

A high-performance and ergonomic timeline tracing library for Rust. Builtin supports [Jaeger](https://crates.io/crates/minitrace-jaeger) and [Datadog](https://crates.io/crates/minitrace-datadog).

## Usage

```toml
[dependencies]
minitrace = "0.3"
minitrace-jaeger = "0.3"
```

```rust
use minitrace::prelude::*;
use futures::executor::block_on;

let (root, collector) = Span::root("root");

{
    let _child_span_1 = Span::enter_with_parent("child span 1", &root);
    // some work
}

drop(root);
let records: Vec<SpanRecord> = block_on(collector.collect());
```

Read the [docs](https://docs.rs/minitrace/) for more details. 

## Examples

### Setup Jaeger

```sh
docker run --rm -d -p6831:6831/udp -p16686:16686 --name jaeger jaegertracing/all-in-one:latest
```

### Run examples

```sh
cargo run --example synchronous
# or
cargo run --example asynchronous
```

Open http://localhost:16686 to see the results.

### Synchronous

![Jaeger Synchronous](img/jaeger-synchronous.png)

### Asynchronous

![Jaeger Asynchronous](img/jaeger-asynchronous.png)

### Benchmark

Benchmark platform is `Intel(R) Xeon(R) CPU E5-2630 v4 @ 2.20GHz` on CentOS 7.

```sh
> cargo criterion

100 spans/Tokio Tracing   time:   [147.64 us 148.80 us 150.25 us]
100 spans/Rustracing      time:   [36.486 us 36.690 us 36.941 us]
100 spans/minitrace       time:   [3.7514 us 3.7564 us 3.7689 us]
```

![Benchmark](img/benchmark.jpeg)
