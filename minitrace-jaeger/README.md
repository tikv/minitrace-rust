# minitrace-jaeger

[![Documentation](https://docs.rs/minitrace-jaeger/badge.svg)](https://docs.rs/minitrace-jaeger/)
[![Crates.io](https://img.shields.io/crates/v/minitrace-jaeger.svg)](https://crates.io/crates/minitrace-jaeger)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

[Jaeger](https://www.jaegertracing.io/) reporter for [`minitrace`](https://crates.io/crates/minitrace).

## Dependencies

```toml
[dependencies]
minitrace = "0.5"
minitrace-jaeger = "0.5"
```

## Setup Jaeger Agent

```sh
docker run --rm -d -p6831:6831/udp -p14268:14268 -p16686:16686 --name jaeger jaegertracing/all-in-one:latest

cargo run --example synchronous
```

Web UI is available on [http://127.0.0.1:16686/](http://127.0.0.1:16686/)

## Report to Jaeger Agent

```rust
use std::net::SocketAddr;

use minitrace::collector::Config;
use minitrace::prelude::*;

// Initialize reporter
let reporter =
    minitrace_jaeger::JaegerReporter::new("127.0.0.1:6831".parse().unwrap(), "asynchronous")
        .unwrap();
minitrace::set_reporter(reporter, Config::default());

{
    // Start tracing
    let root = Span::root("root", SpanContext::new(TraceId(42), SpanId::default()));
}

minitrace::flush();
```
