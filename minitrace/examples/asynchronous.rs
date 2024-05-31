// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![allow(clippy::new_without_default)]

use std::borrow::Cow;
use std::time::Duration;

use minitrace::collector::Config;
use minitrace::collector::Reporter;
use minitrace::prelude::*;
use opentelemetry_otlp::WithExportConfig;

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
    minitrace::set_reporter(ReportAll::new(), Config::default());

    {
        let parent = SpanContext::random();
        let span = Span::root("root", parent);

        let f = async {
            let jhs = {
                let _span = LocalSpan::enter_with_local_parent("a span")
                    .with_property(|| ("a property", "a value"));
                parallel_job()
            };

            other_job().await;

            for jh in jhs {
                jh.await.unwrap();
            }
        }
        .in_span(span);

        tokio::spawn(f).await.unwrap();
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
                "asynchronous",
            )
            .unwrap(),
            datadog: minitrace_datadog::DatadogReporter::new(
                "127.0.0.1:8126".parse().unwrap(),
                "asynchronous",
                "db",
                "select",
            ),
            opentelemetry: minitrace_opentelemetry::OpenTelemetryReporter::new(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint("http://127.0.0.1:4317".to_string())
                    .with_protocol(opentelemetry_otlp::Protocol::Grpc)
                    .with_timeout(Duration::from_secs(
                        opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT,
                    ))
                    .build_span_exporter()
                    .expect("initialize oltp exporter"),
                opentelemetry::trace::SpanKind::Server,
                Cow::Owned(opentelemetry_sdk::Resource::new([
                    opentelemetry::KeyValue::new("service.name", "asynchronous(opentelemetry)"),
                ])),
                opentelemetry::InstrumentationLibrary::builder("example-crate")
                    .with_version(env!("CARGO_PKG_VERSION"))
                    .build(),
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
