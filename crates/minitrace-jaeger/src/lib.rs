// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::{DefaultClock, Span};
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
        spans: Vec<Span>,
    ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync + 'static>> {
        let anchor = DefaultClock::anchor();
        let bn = EmitBatchNotification {
            batch: Batch {
                process: Process {
                    service_name,
                    tags: vec![],
                },
                spans: spans
                    .into_iter()
                    .map(|s| {
                        let begin_cycles = DefaultClock::cycle_to_realtime(s.begin_cycle, anchor);
                        let end_time = DefaultClock::cycle_to_realtime(s.end_cycle, anchor);
                        JaegerSpan {
                            trace_id_low: trace_id as i64,
                            trace_id_high: 0,
                            span_id: s.id.0 as i64,
                            parent_span_id: s.parent_id.0 as i64,
                            operation_name: s.event.to_string(),
                            references: vec![SpanRef {
                                kind: SpanRefKind::FollowsFrom,
                                trace_id_low: trace_id as i64,
                                trace_id_high: 0,
                                span_id: s.parent_id.0 as i64,
                            }],
                            flags: 1,
                            start_time: (begin_cycles.epoch_time_ns / 1_000) as i64,
                            duration: ((end_time.epoch_time_ns - begin_cycles.epoch_time_ns)
                                / 1_000) as i64,
                            tags: s
                                .properties
                                .into_iter()
                                .map(|p| Tag::String {
                                    key: p.0.to_owned(),
                                    value: p.1,
                                })
                                .collect(),
                            logs: vec![],
                        }
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
        spans: Vec<Span>,
    ) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
        let local_addr: SocketAddr = if self.agent.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        }
        .parse()?;

        let udp = UdpSocket::bind(local_addr)?;
        let bytes = Self::encode(self.service_name.to_string(), trace_id, spans)?;
        udp.send_to(&bytes, self.agent)?;

        Ok(())
    }
}
