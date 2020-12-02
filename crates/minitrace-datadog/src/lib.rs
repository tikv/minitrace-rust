// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::Span;
use rmp_serde::Serializer;
use serde::Serialize;
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;

pub struct Reporter;

impl Reporter {
    pub fn encode(
        service_name: &str,
        trace_id: u64,
        root_parent_span_id: u64,
        span_id_prefix: u32,
        spans: &[Span],
    ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync + 'static>> {
        let spans = spans.iter().map(|s| MPSpan {
            name: s.event,
            service: service_name,
            start: s.begin_unix_time_ns as i64,
            duration: s.duration_ns as i64,
            meta: if s.properties.is_empty() {
                None
            } else {
                Some(s.properties.iter().map(|(k, v)| (*k, v.as_ref())).collect())
            },
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
            .post(&format!("http://{}/v0.4/traces", agent))
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
}

#[derive(Serialize)]
struct MPSpan<'a> {
    name: &'a str,
    service: &'a str,
    start: i64,
    duration: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<HashMap<&'a str, &'a str>>,
    span_id: u64,
    trace_id: u64,
    parent_id: u64,
}
