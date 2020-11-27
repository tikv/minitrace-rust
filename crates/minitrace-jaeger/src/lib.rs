// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::Span;
use rustracing_jaeger::thrift::agent::EmitBatchNotification;
use rustracing_jaeger::thrift::jaeger::{
    Batch, Process, Span as JaegerSpan, SpanRef, SpanRefKind, Tag,
};
use std::error::Error;
use std::net::{SocketAddr, UdpSocket};
use thrift_codec::message::Message;
use thrift_codec::CompactEncode;

pub struct Reporter {
    agent: SocketAddr,
    service_name: &'static str,
}

impl Reporter {
    pub fn new(agent: SocketAddr, service_name: &'static str) -> Self {
        Reporter {
            agent,
            service_name,
        }
    }

    pub fn encode(
        service_name: String,
        trace_id: u64,
        root_parent_span_id: u64,
        span_id_prefix: u32,
        spans: &[Span],
    ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync + 'static>> {
        let bn = EmitBatchNotification {
            batch: Batch {
                process: Process {
                    service_name,
                    tags: vec![],
                },
                spans: spans
                    .iter()
                    .map(|s| JaegerSpan {
                        trace_id_low: trace_id as i64,
                        trace_id_high: 0,
                        span_id: span_id_prefix as i64 | s.id as i64,
                        parent_span_id: if s.parent_id == 0 {
                            root_parent_span_id as i64
                        } else {
                            span_id_prefix as i64 | s.parent_id as i64
                        },
                        operation_name: s.event.to_string(),
                        references: vec![SpanRef {
                            kind: SpanRefKind::FollowsFrom,
                            trace_id_low: trace_id as i64,
                            trace_id_high: 0,
                            span_id: s.parent_id as i64,
                        }],
                        flags: 1,
                        start_time: (s.begin_unix_time_ns / 1_000) as i64,
                        duration: (s.duration_ns / 1_000) as i64,
                        tags: s
                            .properties
                            .iter()
                            .map(|p| Tag::String {
                                key: p.0.to_owned(),
                                value: p.1.to_owned(),
                            })
                            .collect(),
                        logs: vec![],
                    })
                    .collect(),
            },
        };

        let mut bytes = Vec::new();
        let msg = Message::from(bn);
        msg.compact_encode(&mut bytes)?;
        Ok(bytes)
    }

    pub fn report(
        &self,
        trace_id: u64,
        span_id_prefix: u32,
        root_parent_span_id: u64,
        spans: &[Span],
    ) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
        let local_addr: SocketAddr = if self.agent.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        }
        .parse()?;

        let udp = UdpSocket::bind(local_addr)?;
        let bytes = Self::encode(
            self.service_name.to_string(),
            trace_id,
            root_parent_span_id,
            span_id_prefix,
            spans,
        )?;
        udp.send_to(&bytes, self.agent)?;

        Ok(())
    }
}
