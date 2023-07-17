// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! Builtin [Datadog](https://docs.datadoghq.com/tracing/) reporter for minitrace.
//!
//! ## Setup Datadog Agent
//!
//! Please follow the Datadog [official documentation](https://docs.datadoghq.com/getting_started/tracing/#datadog-agent).
//!
//! ## Report to Datadog Agent
//!
//! ```no_run
//! use std::net::SocketAddr;
//!
//! use minitrace::collector::Config;
//! use minitrace::prelude::*;
//!
//! // Initialize reporter
//! let reporter = minitrace_datadog::DatadogReporter::new(
//!     "127.0.0.1:8126".parse().unwrap(),
//!     "asynchronous",
//!     "db",
//!     "select",
//! );
//! minitrace::set_reporter(reporter, Config::default());
//!
//! // Start trace
//! let root = Span::root("root", SpanContext::new(TraceId(42), SpanId::default()));
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;

use minitrace::collector::Reporter;
use minitrace::prelude::*;
use rmp_serde::Serializer;
use serde::Serialize;

pub struct DatadogReporter {
    agent_addr: SocketAddr,
    service_name: String,
    resource: String,
    trace_type: String,
}

impl DatadogReporter {
    pub fn new(
        agent_addr: SocketAddr,
        service_name: impl Into<String>,
        resource: impl Into<String>,
        trace_type: impl Into<String>,
    ) -> DatadogReporter {
        DatadogReporter {
            agent_addr,
            service_name: service_name.into(),
            resource: resource.into(),
            trace_type: trace_type.into(),
        }
    }

    fn convert<'a>(&'a self, spans: &'a [SpanRecord]) -> Vec<DatadogSpan<'a>> {
        spans
            .iter()
            .map(move |s| DatadogSpan {
                name: s.name,
                service: &self.service_name,
                trace_type: &self.trace_type,
                resource: &self.resource,
                start: s.begin_unix_time_ns as i64,
                duration: s.duration_ns as i64,
                meta: if s.properties.is_empty() {
                    None
                } else {
                    Some(s.properties.iter().map(|(k, v)| (*k, v.as_ref())).collect())
                },
                error_code: 0,
                span_id: s.span_id.0,
                trace_id: s.trace_id.0 as u64,
                parent_id: s.parent_id.0,
            })
            .collect()
    }

    fn serialize(&self, spans: Vec<DatadogSpan>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut buf = vec![0b10010001];
        spans.serialize(&mut Serializer::new(&mut buf).with_struct_map())?;
        Ok(buf)
    }
}

impl Reporter for DatadogReporter {
    fn report(&mut self, spans: &[SpanRecord]) -> Result<(), Box<dyn std::error::Error>> {
        let datadog_spans = self.convert(&spans);
        if let Ok(bytes) = self.serialize(datadog_spans) {
            let client = reqwest::blocking::Client::new();
            let _rep = client
                .post(format!("http://{}/v0.4/traces", self.agent_addr))
                .header("Datadog-Meta-Tracer-Version", "v1.27.0")
                .header("Content-Type", "application/msgpack")
                .body(bytes)
                .send()?;
        }
        Ok(())
    }
}
#[derive(Serialize)]
struct DatadogSpan<'a> {
    name: &'a str,
    service: &'a str,
    #[serde(rename = "type")]
    trace_type: &'a str,
    resource: &'a str,
    start: i64,
    duration: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<HashMap<&'a str, &'a str>>,
    error_code: i32,
    span_id: u64,
    trace_id: u64,
    parent_id: u64,
}
