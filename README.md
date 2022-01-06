# minitrace

[![Actions Status](https://github.com/tikv/minitrace-rust/workflows/CI/badge.svg)](https://github.com/tikv/minitrace-rust/actions)
[![Documentation](https://docs.rs/minitrace/badge.svg)](https://docs.rs/minitrace/)
[![Crates.io](https://img.shields.io/crates/v/minitrace.svg)](https://crates.io/crates/minitrace)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

A high-performance and ergonomic timeline tracing library for Rust. Builtin supports [Jaeger](https://www.jaegertracing.io/) and [Datadog](https://www.datadoghq.com/).

## Usage

```toml
[dependencies]
minitrace = "0.2"
minitrace-jaeger = "0.2"
```

```rust
use minitrace::prelude::*;

let (root, collector) = Span::root("root");

{
    let _child_span_1 = Span::enter_with_parent("child span 1", &root);
    // some work
}

drop(root);
let records: Vec<SpanRecord> = collector.collect();
```

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
