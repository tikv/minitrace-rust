# minitrace-datadog

[![Documentation](https://docs.rs/minitrace-datadog/badge.svg)](https://docs.rs/minitrace-datadog/)
[![Crates.io](https://img.shields.io/crates/v/minitrace-datadog.svg)](https://crates.io/crates/minitrace-datadog)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

[Datadog](https://docs.datadoghq.com/tracing/) reporter for [`minitrace`](https://crates.io/crates/minitrace).

## Dependencies

```toml
[dependencies]
minitrace = "0.6"
minitrace-datadog = "0.6"
```

## Setup Datadog Agent

Please follow the Datadog [official documentation](https://docs.datadoghq.com/getting_started/tracing/#datadog-agent).

```sh
cargo run --example synchronous
```

## Report to Datadog Agent

```rust
use std::net::SocketAddr;

use minitrace::collector::Config;
use minitrace::prelude::*;

// Initialize reporter
let reporter = minitrace_datadog::DatadogReporter::new(
    "127.0.0.1:8126".parse().unwrap(),
    "asynchronous",
    "db",
    "select",
);
minitrace::set_reporter(reporter, Config::default());

{
    // Start tracing
    let root = Span::root("root", SpanContext::random());
}

minitrace::flush();
```
