# minitrace-jaeger

[![Documentation](https://docs.rs/minitrace-jaeger/badge.svg)](https://docs.rs/minitrace-jaeger/)
[![Crates.io](https://img.shields.io/crates/v/minitrace-jaeger.svg)](https://crates.io/crates/minitrace-jaeger)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

Builtin [Jaeger](https://www.jaegertracing.io/) reporter for minitrace.

## Dependencies

```toml
[dependencies]
minitrace = "0.4"
minitrace-jaeger = "0.4"
```

## Setup Jaeger Agent

```sh
docker run --rm -d -p6831:6831/udp -p16686:16686 --name jaeger jaegertracing/all-in-one:latest
```

Web UI is available on http://127.0.0.1:16686/

## Report to Jaeger Agent

```rust
use std::net::SocketAddr;

use futures::executor::block_on;
use minitrace::prelude::*;

// start trace
let (root_span, collector) = Span::root("root");

// finish trace
drop(root_span);

// collect spans
let spans = block_on(collector.collect());

// encode trace
const TRACE_ID: u64 = 42;
const SPAN_ID_PREFIX: u32 = 42;
const ROOT_PARENT_SPAN_ID: u64 = 0;
let bytes = minitrace_jaeger::encode(
    String::from("service name"),
    TRACE_ID,
    ROOT_PARENT_SPAN_ID,
    SPAN_ID_PREFIX,
    &spans,
)
.expect("encode error");

// report trace
let socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 6831);
minitrace_jaeger::report_blocking(socket, &bytes).expect("report error");
```
