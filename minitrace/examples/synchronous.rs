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
    let bytes =
        minitrace_jaeger::encode("synchronous".to_owned(), rand::random(), 0, 0, &spans).unwrap();
    minitrace_jaeger::report("127.0.0.1:6831".parse().unwrap(), &bytes)
        .await
        .ok();

    // Report to Datadog
    let bytes = minitrace_datadog::encode(
        "synchronous",
        "web",
        "/health",
        0,
        rand::random(),
        0,
        0,
        &spans,
    )
    .unwrap();
    minitrace_datadog::report("127.0.0.1:8126".parse().unwrap(), bytes)
        .await
        .ok();

    // Report to OpenTelemetry
    let instrumentation_lib = opentelemetry::InstrumentationLibrary::new(
        "my-crate",
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
            opentelemetry::KeyValue::new("service.name", "synchronous"),
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
