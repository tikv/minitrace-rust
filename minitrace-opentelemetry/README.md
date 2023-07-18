# minitrace-opentelemetry

[![Documentation](https://docs.rs/minitrace-opentelemetry/badge.svg)](https://docs.rs/minitrace-opentelemetry/)
[![Crates.io](https://img.shields.io/crates/v/minitrace-opentelemetry.svg)](https://crates.io/crates/minitrace-opentelemetry)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

[OpenTelemetry](https://github.com/open-telemetry/opentelemetry-rust) reporter for [`minitrace`](https://crates.io/crates/minitrace).

## Dependencies

```toml
[dependencies]
minitrace = "0.4"
minitrace-opentelemetry = "0.4"
```

## Setup OpenTelemetry Collector

```sh
cd minitrace-opentelemetry/examples
docker compose up -d

cargo run --example synchronous
```

Jaeger UI is available on [http://127.0.0.1:16686/](http://127.0.0.1:16686/)

Zipkin UI is available on [http://127.0.0.1:9411/](http://127.0.0.1:16686/)

## Report to OpenTelemetry Collector

```rust, no_run
use std::borrow::Cow;
use std::time::Duration;
use minitrace::collector::Config;
use minitrace::prelude::*;
use minitrace_opentelemetry::OpenTelemetryReporter;
use opentelemetry_otlp::{SpanExporter, ExportConfig, Protocol, TonicConfig};
use opentelemetry::trace::SpanKind;
use opentelemetry::sdk::Resource;
use opentelemetry::KeyValue;
use opentelemetry::InstrumentationLibrary;

// Initialize reporter
let reporter = OpenTelemetryReporter::new(
    SpanExporter::new_tonic(
        ExportConfig {
            endpoint: "http://127.0.0.1:4317".to_string(),
            protocol: Protocol::Grpc,
            timeout: Duration::from_secs(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT),
        },
        TonicConfig::default(),
    )
    .unwrap(),
    SpanKind::Server,
    Cow::Owned(Resource::new([KeyValue::new("service.name", "asynchronous")])),
    InstrumentationLibrary::new("example-crate", Some(env!("CARGO_PKG_VERSION")), None),
);
minitrace::set_reporter(reporter, Config::default());

{
    // Start tracing
    let root = Span::root("root", SpanContext::new(TraceId(42), SpanId::default()));
}

minitrace::flush()
```
