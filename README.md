# Minitrace
[![Actions Status](https://github.com/tikv/minitrace-rust/workflows/CI/badge.svg)](https://github.com/tikv/minitrace-rust/actions)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

A high-performance, ergonomic timeline tracing library for Rust. Builtin supports [Jaeger](https://www.jaegertracing.io/) and [Datadog])(https://www.datadoghq.com/).

## Usage

```toml
[dependencies]
minitrace = { git = "https://github.com/tikv/minitrace-rust.git" }
minitrace-jaeger = { git = "https://github.com/tikv/minitrace-rust.git" }
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
