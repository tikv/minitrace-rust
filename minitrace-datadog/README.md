# minitrace-datadog

[![Documentation](https://docs.rs/minitrace-datadog/badge.svg)](https://docs.rs/minitrace-datadog/)
[![Crates.io](https://img.shields.io/crates/v/minitrace-datadog.svg)](https://crates.io/crates/minitrace-datadog)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

Builtin [Datadog](https://docs.datadoghq.com/tracing/) reporter for minitrace.

## Dependencies

```toml
[dependencies]
minitrace = "0.4"
minitrace-datadog = "0.4"
```

## Setup Datadog Agent

Please follow the Datadog [official documentation](https://docs.datadoghq.com/getting_started/tracing/#datadog-agent).

## Report to Datadog Agent

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
const ERROR_CODE: i32 = 0;
const TRACE_ID: u64 = 42;
const SPAN_ID_PREFIX: u32 = 42;
const ROOT_PARENT_SPAN_ID: u64 = 0;
let bytes = minitrace_datadog::encode(
    "service_name",
    "trace_type",
    "resource",
    ERROR_CODE,
    TRACE_ID,
    ROOT_PARENT_SPAN_ID,
    SPAN_ID_PREFIX,
    &spans,
)
.expect("encode error");

// report trace
let socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 8126);
minitrace_datadog::report_blocking(socket, bytes).expect("report error");
```
