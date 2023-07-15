# minitrace-opentelemetry

[![Documentation](https://docs.rs/minitrace-opentelemetry/badge.svg)](https://docs.rs/minitrace-opentelemetry/)
[![Crates.io](https://img.shields.io/crates/v/minitrace-opentelemetry.svg)](https://crates.io/crates/minitrace-opentelemetry)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

Builtin [OpenTelemetry OTLP](https://github.com/open-telemetry/opentelemetry-collector) reporter for minitrace.

## Dependencies

```toml
[dependencies]
minitrace = "0.4"
minitrace-opentelemetry = "0.4"
```

## Setup OpenTelemetry Collector

```sh
cd examples
docker compose up -d
```

Jaeger UI is available on http://127.0.0.1:16686/
Zipkin UI is available on http://127.0.0.1:9411/

## Report to OpenTelemetry Collector

```rust
use std::time::Duration;

use minitrace::prelude::*;
use opentelemetry::sdk::export::trace::SpanExporter as _;

// start trace
let (root_span, collector) = Span::root("root");

// finish trace
drop(root_span);

// collect spans
let spans = collector.collect().await;

// report trace
let instrumentation_lib = opentelemetry::InstrumentationLibrary::new(
    "example-crate",
    Some(env!("CARGO_PKG_VERSION")),
    None,
);
let otlp_spans = minitrace_opentelemetry::convert(
    &spans,
    0,
    rand::random(),
    0u64.to_le_bytes(),
    opentelemetry::trace::TraceState::default(),
    opentelemetry::trace::Status::Ok,
    opentelemetry::trace::SpanKind::Server,
    true,
    std::borrow::Cow::Owned(opentelemetry::sdk::Resource::new([
        opentelemetry::KeyValue::new("service.name", "example"),
    ])),
    instrumentation_lib,
)
.collect();
let mut exporter = opentelemetry_otlp::SpanExporter::new_tonic(
    opentelemetry_otlp::ExportConfig {
        endpoint: "http://127.0.0.1:4317".to_string(),
        protocol: opentelemetry_otlp::Protocol::Grpc,
        timeout: Duration::from_secs(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT),
    },
    opentelemetry_otlp::TonicConfig::default(),
)
.unwrap();
exporter.export(otlp_spans).await.ok();
exporter.force_flush().await.ok();
```
