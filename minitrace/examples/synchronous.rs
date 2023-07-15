// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::Duration;

use minitrace::prelude::*;
use opentelemetry::sdk::export::trace::SpanExporter as _;

fn func1(i: u64) {
    let _guard = LocalSpan::enter_with_local_parent("func1");
    std::thread::sleep(Duration::from_millis(i));
    func2(i);
}

#[trace]
fn func2(i: u64) {
    std::thread::sleep(Duration::from_millis(i));
}

#[tokio::main]
async fn main() {
    let collector = {
        let (span, collector) = Span::root("root");

        let _g = span.set_local_parent();
        let mut span = LocalSpan::enter_with_local_parent("a span");
        span.add_property(|| ("a property", "a value".to_owned()));

        for i in 1..=10 {
            func1(i);
        }

        collector
    };

    let spans = collector.collect().await;

    // Report to Jaeger
    let jaeger_spans = minitrace_jaeger::convert(&spans, 0, rand::random(), 0).collect();
    minitrace_jaeger::report(
        "synchronous".to_string(),
        "127.0.0.1:6831".parse().unwrap(),
        jaeger_spans,
    )
    .await
    .ok();

    // Report to Datadog
    let datadog_spans = minitrace_datadog::convert(
        &spans,
        0,
        rand::random(),
        0,
        "synchronous",
        "web",
        "/health",
        0,
    )
    .collect();
    minitrace_datadog::report("127.0.0.1:8126".parse().unwrap(), datadog_spans)
        .await
        .ok();

    // Report to OpenTelemetry
    let instrumentation_lib = opentelemetry::InstrumentationLibrary::new(
        "my-crate",
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
            opentelemetry::KeyValue::new("service.name", "synchronous"),
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
}
