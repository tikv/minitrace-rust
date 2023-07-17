// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;
use std::time::Duration;

use minitrace::collector::Config;
use minitrace::prelude::*;

#[tokio::main]
async fn main() {
    // Set reporter
    let reporter = minitrace_opentelemetry::OpenTelemetryReporter::new(
        opentelemetry_otlp::SpanExporter::new_tonic(
            opentelemetry_otlp::ExportConfig {
                endpoint: "http://127.0.0.1:4317".to_string(),
                protocol: opentelemetry_otlp::Protocol::Grpc,
                timeout: Duration::from_secs(
                    opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT,
                ),
            },
            opentelemetry_otlp::TonicConfig::default(),
        )
        .unwrap(),
        opentelemetry::trace::SpanKind::Server,
        Cow::Owned(opentelemetry::sdk::Resource::new([
            opentelemetry::KeyValue::new("service.name", "example"),
        ])),
        opentelemetry::InstrumentationLibrary::new(
            "example-crate",
            Some(env!("CARGO_PKG_VERSION")),
            None,
        ),
    );
    minitrace::set_reporter(reporter, Config::default());

    // Start trace
    let ctx = SpanContext::new(TraceId(rand::random()), SpanId::default());
    let root = Span::root("root", ctx);

    // Finish trace
    drop(root);

    // Wait for the reporter to finish the last batch
    minitrace::flush();
}
