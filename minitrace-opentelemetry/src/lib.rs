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
//! use std::time::Duration;
//!
//! use minitrace::prelude::*;
//! use opentelemetry::sdk::export::trace::SpanExporter as _;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // start trace
//! let (root_span, collector) = Span::root("root");
//!
//! // finish trace
//! drop(root_span);
//!
//! // collect spans
//! let spans = collector.collect().await;
//!
//! // report trace
//! let instrumentation_lib = opentelemetry::InstrumentationLibrary::new(
//!     "example-crate",
//!     Some(env!("CARGO_PKG_VERSION")),
//!     None,
//! );
//! let span_data = minitrace_opentelemetry::convert(
//!     rand::random(),
//!     opentelemetry::trace::TraceState::default(),
//!     opentelemetry::trace::Status::Ok,
//!     opentelemetry::trace::SpanKind::Server,
//!     true,
//!     std::borrow::Cow::Owned(opentelemetry::sdk::Resource::new([
//!         opentelemetry::KeyValue::new("service.name", "example"),
//!     ])),
//!     instrumentation_lib,
//!     0u64.to_le_bytes(),
//!     0,
//!     &spans,
//! );
//! let mut exporter = opentelemetry_otlp::SpanExporter::new_tonic(
//!     opentelemetry_otlp::ExportConfig {
//!         endpoint: "http://127.0.0.1:4317".to_string(),
//!         protocol: opentelemetry_otlp::Protocol::Grpc,
//!         timeout: Duration::from_secs(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT),
//!     },
//!     opentelemetry_otlp::TonicConfig::default(),
//! )
//! .unwrap();
//! exporter.export(span_data).await.unwrap();
//! exporter.force_flush().await.unwrap();
//! # }

use std::borrow::Cow;
use std::time::Duration;
use std::time::UNIX_EPOCH;

use minitrace::collector::EventRecord;
use minitrace::prelude::*;
use opentelemetry::sdk::export::trace::SpanData;
use opentelemetry::sdk::trace::EvictedHashMap;
use opentelemetry::sdk::trace::EvictedQueue;
use opentelemetry::sdk::Resource;
use opentelemetry::trace::Event;
use opentelemetry::trace::SpanContext;
use opentelemetry::trace::SpanId;
use opentelemetry::trace::SpanKind;
use opentelemetry::trace::Status;
use opentelemetry::trace::TraceFlags;
use opentelemetry::trace::TraceState;
use opentelemetry::InstrumentationLibrary;
use opentelemetry::KeyValue;

#[allow(clippy::too_many_arguments)]
pub fn convert(
    trace_id: [u8; 16],
    trace_state: TraceState,
    status: Status,
    span_kind: SpanKind,
    sampled: bool,
    resource: Cow<'static, Resource>,
    instrumentation_lib: InstrumentationLibrary,
    root_parent_span_id: [u8; 8],
    span_id_prefix: u32,
    spans: &[SpanRecord],
) -> Vec<SpanData> {
    spans
        .iter()
        .map(|span| SpanData {
            span_context: SpanContext::new(
                opentelemetry::trace::TraceId::from_bytes(trace_id),
                SpanId::from_bytes(((span_id_prefix as u64) << 32 | span.id as u64).to_le_bytes()),
                TraceFlags::default().with_sampled(sampled),
                false,
                trace_state.clone(),
            ),
            parent_span_id: if span.parent_id == 0 {
                SpanId::from_bytes(root_parent_span_id)
            } else {
                SpanId::from_bytes(
                    ((span_id_prefix as u64) << 32 | span.parent_id as u64).to_le_bytes(),
                )
            },
            name: span.name.into(),
            start_time: UNIX_EPOCH + Duration::from_nanos(span.begin_unix_time_ns),
            end_time: UNIX_EPOCH + Duration::from_nanos(span.begin_unix_time_ns + span.duration_ns),
            attributes: convert_properties(&span.properties),
            events: convert_events(&span.events),
            links: EvictedQueue::new(0),
            status: status.clone(),
            span_kind: span_kind.clone(),
            resource: resource.clone(),
            instrumentation_lib: instrumentation_lib.clone(),
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
