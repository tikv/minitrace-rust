// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::prelude::*;

fn parallel_job() -> Vec<tokio::task::JoinHandle<()>> {
    let mut v = Vec::with_capacity(4);
    for i in 0..4 {
        v.push(tokio::spawn(
            iter_job(i).in_span(Span::enter_with_local_parent("iter job")),
        ));
    }
    v
}

async fn iter_job(iter: u64) {
    std::thread::sleep(std::time::Duration::from_millis(iter * 10));
    tokio::task::yield_now().await;
    other_job().await;
}

#[trace(enter_on_poll = true)]
async fn other_job() {
    for i in 0..20 {
        if i == 10 {
            tokio::task::yield_now().await;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[tokio::main]
async fn main() {
    let (span, collector) = Span::root("root");

    let f = async {
        let jhs = {
            let mut span = LocalSpan::enter_with_local_parent("a span");
            span.add_property(|| ("a property", "a value".to_owned()));
            parallel_job()
        };

        other_job().await;

        for jh in jhs {
            jh.await.unwrap();
        }
    }
    .in_span(span);

    tokio::spawn(f).await.unwrap();

    let spans = collector.collect().await;

    // Report to Jaeger
    let bytes =
        minitrace_jaeger::encode("asynchronous".to_owned(), rand::random(), 0, 0, &spans).unwrap();
    minitrace_jaeger::report("127.0.0.1:6831".parse().unwrap(), &bytes)
        .await
        .ok();

    // Report to Datadog
    let bytes = minitrace_datadog::encode(
        "asynchronous",
        "db",
        "select",
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
            opentelemetry::KeyValue::new("service.name", "asynchronous"),
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
    exporter.export(span_data).await.ok();
    exporter.force_flush().await.ok();
}
