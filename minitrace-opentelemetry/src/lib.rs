// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

//! Builtin [OpenTelemetry OTLP](https://github.com/open-telemetry/opentelemetry-collector) reporter for minitrace.
//!
//! ## Setup OpenTelemetry Collector
//!
//! ```sh
//! cd examples
//! docker compose up -d
//! ```
//!
//! Jaeger UI is available on http://127.0.0.1:16686/
//! Zipkin UI is available on http://127.0.0.1:9411/
//!
//! ## Report to OpenTelemetry Collector
//!
//! ```no_run
//! use std::borrow::Cow;
//! use std::time::Duration;
//!
//! use minitrace::collector::Config;
//! use minitrace::prelude::*;
//!
//! // Initialize reporter
//! let reporter = minitrace_opentelemetry::OpenTelemetryReporter::new(
//!     opentelemetry_otlp::SpanExporter::new_tonic(
//!         opentelemetry_otlp::ExportConfig {
//!             endpoint: "http://127.0.0.1:4317".to_string(),
//!             protocol: opentelemetry_otlp::Protocol::Grpc,
//!             timeout: Duration::from_secs(
//!                 opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT,
//!             ),
//!         },
//!         opentelemetry_otlp::TonicConfig::default(),
//!     )
//!     .unwrap(),
//!     opentelemetry::trace::SpanKind::Server,
//!     Cow::Owned(opentelemetry::sdk::Resource::new([
//!         opentelemetry::KeyValue::new("service.name", "asynchronous"),
//!     ])),
//!     opentelemetry::InstrumentationLibrary::new(
//!         "example-crate",
//!         Some(env!("CARGO_PKG_VERSION")),
//!         None,
//!     ),
//! );
//! minitrace::set_reporter(reporter, Config::default());
//!
//! // Start trace
//! let root = Span::root("root", SpanContext::new(TraceId(42), SpanId::default()));
//! ```

use std::borrow::Cow;
use std::time::Duration;
use std::time::UNIX_EPOCH;

use minitrace::collector::EventRecord;
use minitrace::collector::Reporter;
use minitrace::prelude::*;
use opentelemetry::sdk::export::trace::SpanData;
use opentelemetry::sdk::export::trace::SpanExporter;
use opentelemetry::sdk::trace::EvictedHashMap;
use opentelemetry::sdk::trace::EvictedQueue;
use opentelemetry::sdk::Resource;
use opentelemetry::trace::Event;
use opentelemetry::trace::SpanContext;
use opentelemetry::trace::SpanKind;
use opentelemetry::trace::Status;
use opentelemetry::trace::TraceFlags;
use opentelemetry::trace::TraceState;
use opentelemetry::InstrumentationLibrary;
use opentelemetry::KeyValue;

pub struct OpenTelemetryReporter {
    opentelemetry_exporter: Box<dyn SpanExporter>,
    span_kind: SpanKind,
    resource: Cow<'static, Resource>,
    instrumentation_lib: InstrumentationLibrary,
}

impl OpenTelemetryReporter {
    pub fn new(
        opentelemetry_exporter: impl SpanExporter + 'static,
        span_kind: SpanKind,
        resource: Cow<'static, Resource>,
        instrumentation_lib: InstrumentationLibrary,
    ) -> Self {
        OpenTelemetryReporter {
            opentelemetry_exporter: Box::new(opentelemetry_exporter),
            span_kind,
            resource,
            instrumentation_lib,
        }
    }

    fn convert(&self, spans: &[SpanRecord]) -> Vec<SpanData> {
        spans
            .iter()
            .map(move |span| SpanData {
                span_context: SpanContext::new(
                    span.trace_id.0.to_be_bytes().into(),
                    span.span_id.0.to_be_bytes().into(),
                    TraceFlags::default(),
                    false,
                    TraceState::default(),
                ),
                parent_span_id: span.parent_id.0.to_be_bytes().into(),
                name: span.name.into(),
                start_time: UNIX_EPOCH + Duration::from_nanos(span.begin_unix_time_ns),
                end_time: UNIX_EPOCH
                    + Duration::from_nanos(span.begin_unix_time_ns + span.duration_ns),
                attributes: Self::convert_properties(&span.properties),
                events: Self::convert_events(&span.events),
                links: EvictedQueue::new(0),
                status: Status::default(),
                span_kind: self.span_kind.clone(),
                resource: self.resource.clone(),
                instrumentation_lib: self.instrumentation_lib.clone(),
            })
            .collect()
    }

    fn convert_properties(properties: &[(&'static str, String)]) -> EvictedHashMap {
        let mut map = EvictedHashMap::new(u32::MAX, properties.len());
        for (k, v) in properties {
            map.insert(KeyValue::new(*k, v.clone()));
        }
        map
    }

    fn convert_events(events: &[EventRecord]) -> EvictedQueue<Event> {
        let mut queue = EvictedQueue::new(u32::MAX);
        queue.extend(events.iter().map(|event| {
            Event::new(
                event.name,
                UNIX_EPOCH + Duration::from_nanos(event.timestamp_unix_ns),
                event
                    .properties
                    .iter()
                    .map(|(k, v)| KeyValue::new(*k, v.clone()))
                    .collect(),
                0,
            )
        }));
        queue
    }

    fn try_report(&mut self, spans: &[SpanRecord]) -> Result<(), Box<dyn std::error::Error>> {
        let opentelemetry_spans = self.convert(spans);
        futures::executor::block_on(self.opentelemetry_exporter.export(opentelemetry_spans))?;
        Ok(())
    }
}

impl Reporter for OpenTelemetryReporter {
    fn report(&mut self, spans: &[SpanRecord]) {
        if spans.is_empty() {
            return;
        }

        if let Err(err) = self.try_report(spans) {
            eprintln!("report to opentelemetry failed: {}", err);
        }
    }
}
