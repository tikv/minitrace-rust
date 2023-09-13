// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![allow(clippy::new_without_default)]

use std::borrow::Cow;
use std::time::Duration;

use minitrace::collector::Config;
use minitrace::collector::Reporter;
use minitrace::prelude::*;

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
    minitrace::set_reporter(ReportAll::new(), Config::default());

    {
        let parent = SpanContext::random();
        let root = Span::root("root", parent);

        let _g = root.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent("a span")
            .with_property(|| ("a property".into(), "a value".into()));

        for i in 1..=10 {
            func1(i);
        }
    }

    minitrace::flush();
}

pub struct ReportAll {
    jaeger: minitrace_jaeger::JaegerReporter,
    datadog: minitrace_datadog::DatadogReporter,
    opentelemetry: minitrace_opentelemetry::OpenTelemetryReporter,
}

impl ReportAll {
    pub fn new() -> ReportAll {
        ReportAll {
            jaeger: minitrace_jaeger::JaegerReporter::new(
                "127.0.0.1:6831".parse().unwrap(),
                "synchronous",
            )
            .unwrap(),
            datadog: minitrace_datadog::DatadogReporter::new(
                "127.0.0.1:8126".parse().unwrap(),
                "synchronous",
                "db",
                "select",
            ),
            opentelemetry: minitrace_opentelemetry::OpenTelemetryReporter::new(
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
                    opentelemetry::KeyValue::new("service.name", "synchronous"),
                ])),
                opentelemetry::InstrumentationLibrary::new(
                    "example-crate",
                    Some(env!("CARGO_PKG_VERSION")),
                    None,
                ),
            ),
        }
    }
}

impl Reporter for ReportAll {
    fn report(&mut self, spans: &[SpanRecord]) {
        self.jaeger.report(spans);
        self.datadog.report(spans);
        self.opentelemetry.report(spans);
    }
}
