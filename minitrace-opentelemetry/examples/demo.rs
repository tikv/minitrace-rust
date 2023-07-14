// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::Duration;

use minitrace::prelude::*;
use opentelemetry::sdk::export::trace::SpanExporter as _;

#[tokio::main]
async fn main() {
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
    let span_data = minitrace_opentelemetry::convert(
        rand::random(),
        opentelemetry::trace::TraceState::default(),
        opentelemetry::trace::Status::Ok,
        opentelemetry::trace::SpanKind::Server,
        true,
        std::borrow::Cow::Owned(opentelemetry::sdk::Resource::new([
            opentelemetry::KeyValue::new("service.name", "example"),
        ])),
        instrumentation_lib,
        0u64.to_le_bytes(),
        0,
        &spans,
    );
    let mut exporter = opentelemetry_otlp::SpanExporter::new_tonic(
        opentelemetry_otlp::ExportConfig {
            endpoint: "http://127.0.0.1:4317".to_string(),
            protocol: opentelemetry_otlp::Protocol::Grpc,
            timeout: Duration::from_secs(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT),
        },
        opentelemetry_otlp::TonicConfig::default(),
    )
    .unwrap();
    exporter.export(span_data).await.unwrap();
    exporter.force_flush().await.unwrap();
}
