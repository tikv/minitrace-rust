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
//! use futures::executor::block_on;
//! use minitrace::prelude::*;
//!
//! // start trace
//! let (root_span, collector) = Span::root("root");
//!
//! // finish trace
//! drop(root_span);
//!
//! // collect spans
//! let spans = block_on(collector.collect());
//!
//! // encode trace
//! const ERROR_CODE: i32 = 0;
//! const TRACE_ID: u64 = 42;
//! const SPAN_ID_PREFIX: u32 = 42;
//! const ROOT_PARENT_SPAN_ID: u64 = 0;
//! let bytes = minitrace_datadog::encode(
//!     "service_name",
//!     "trace_type",
//!     "resource",
//!     ERROR_CODE,
//!     TRACE_ID,
//!     ROOT_PARENT_SPAN_ID,
//!     SPAN_ID_PREFIX,
//!     &spans,
//! )
//! .expect("encode error");
//!
//! // report trace
//! let socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 8126);
//! minitrace_datadog::report_blocking(socket, bytes).expect("report error");
//! ```

use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;

use minitrace::prelude::*;
use rmp_serde::Serializer;
use serde::Serialize;

#[allow(clippy::too_many_arguments)]
pub fn encode(
    service_name: &str,
    trace_type: &str,
    resource: &str,
    error_code: i32,
    trace_id: u64,
    root_parent_span_id: u64,
    span_id_prefix: u32,
    spans: &[SpanRecord],
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync + 'static>> {
    let spans = spans.iter().map(|s| MPSpan {
        name: s.name,
        service: service_name,
        trace_type,
        resource,
        start: s.begin_unix_time_ns as i64,
        duration: s.duration_ns as i64,
        meta: if s.properties.is_empty() {
            None
        } else {
            Some(s.properties.iter().map(|(k, v)| (*k, v.as_ref())).collect())
        },
        error_code,
        span_id: (span_id_prefix as u64) << 32 | s.id as u64,
        trace_id,
        parent_id: if s.parent_id == 0 {
            root_parent_span_id
        } else {
            (span_id_prefix as u64) << 32 | s.parent_id as u64
        },
    });

    let mut buf = vec![0b10010001];
    spans
        .collect::<Vec<_>>()
        .serialize(&mut Serializer::new(&mut buf).with_struct_map())?;

    Ok(buf)
}

pub fn report_blocking(
    agent: SocketAddr,
    bytes: Vec<u8>,
) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let client = reqwest::blocking::Client::new();
    let rep = client
        .post(format!("http://{}/v0.4/traces", agent))
        .header("Datadog-Meta-Tracer-Version", "v1.27.0")
        .header("Content-Type", "application/msgpack")
        .body(bytes)
        .send()?;

    if rep.status().as_u16() >= 400 {
        let status = rep.status();
        return Err(format!("{} (Status: {})", rep.text()?, status).into());
    }

    Ok(())
}

pub async fn report(
    agent: SocketAddr,
    bytes: Vec<u8>,
) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let client = reqwest::Client::new();
    let rep = client
        .post(&format!("http://{}/v0.4/traces", agent))
        .header("Datadog-Meta-Tracer-Version", "v1.27.0")
        .header("Content-Type", "application/msgpack")
        .body(bytes)
        .send()
        .await?;

    if rep.status().as_u16() >= 400 {
        let status = rep.status();
        return Err(format!("{} (Status: {})", rep.text().await?, status).into());
    }

    Ok(())
}

#[derive(Serialize)]
struct MPSpan<'a> {
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
