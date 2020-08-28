// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::HashMap;
use std::sync::Mutex;

use crossbeam::queue::SegQueue;
use lazy_static::lazy_static;

use crate::trace::Span;
use crate::utils::real_time_ns;

const INIT_LEN: usize = 1024;
const INIT_BYTES_LEN: usize = 16384;

lazy_static! {
    pub static ref SPAN_COLLECTOR: SegQueue<(u32, SpanSet)> = SegQueue::new();
    pub static ref COLLECTED: Mutex<HashMap<u32, SpanSet>> = Mutex::new(HashMap::new());
}

pub fn collect_by_trace_id(trace_id: u32) -> Option<TraceResult> {
    let mut collected = COLLECTED.lock().unwrap();
    collect_and_merge(&mut collected);
    let span_set = collected.remove(&trace_id)?;

    Some(TraceResult {
        baseline_cycles: minstant::now(),
        baseline_ns: real_time_ns(),
        cycles_per_second: minstant::cycles_per_second(),
        spans: span_set.spans,
        properties: span_set.properties,
    })
}

pub fn collect_all() -> HashMap<u32, TraceResult> {
    let mut collected = COLLECTED.lock().unwrap();
    collect_and_merge(&mut collected);
    collected
        .drain()
        .map(|(trace_id, span_set)| {
            (
                trace_id,
                TraceResult {
                    baseline_cycles: minstant::now(),
                    baseline_ns: real_time_ns(),
                    cycles_per_second: minstant::cycles_per_second(),
                    spans: span_set.spans,
                    properties: span_set.properties,
                },
            )
        })
        .collect()
}

fn collect_and_merge(collected: &mut HashMap<u32, SpanSet>) {
    while let Ok((other_trace_id, other_span_set)) = SPAN_COLLECTOR.pop() {
        let span_set = collected.entry(other_trace_id).or_insert(SpanSet::new());
        span_set.append(other_span_set);
    }
}

#[derive(Debug, Clone)]
pub struct TraceResult {
    pub baseline_cycles: u64,
    pub baseline_ns: u64,

    /// For conversion of cycles -> ns
    pub cycles_per_second: u64,

    /// Span collection
    pub spans: Vec<Span>,

    /// Properties
    pub properties: Properties,
}

/// Properties can used to attach some information about tracing context
/// to current span, e.g. host of the request, CPU usage.
///
/// Usage:
/// ```
/// # let event_id = 1u32;
/// let _guard = minitrace::new_span(event_id);
/// minitrace::new_property(b"host:127.0.0.1");
/// minitrace::new_property(b"cpu_usage:42%");
/// ```
///
/// Every property will relate to a span. Logically properties are a sequence
/// of (span id, property) pairs:
/// ```text
/// span id -> property
/// 10      -> b"123"
/// 10      -> b"!@$#$%"
/// 12      -> b"abcd"
/// 14      -> b"xyz"
/// ```
///
/// and will be stored into `Properties` struct as:
/// ```text
/// span_ids: [10, 10, 12, 14]
/// property_lens: [3, 6, 4, 3]
/// payload: b"123!@$#$%abcdxyz"
/// ```
#[derive(Debug, Clone)]
pub struct Properties {
    pub span_ids: Vec<u32>,
    pub property_lens: Vec<u64>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SpanSet {
    /// Span collection
    pub spans: Vec<Span>,

    /// Property collection
    pub properties: Properties,
}

impl SpanSet {
    pub fn new() -> Self {
        SpanSet {
            spans: Vec::new(),
            properties: Properties {
                span_ids: Vec::new(),
                property_lens: Vec::new(),
                payload: Vec::new(),
            },
        }
    }

    pub fn with_capacity() -> Self {
        SpanSet {
            spans: Vec::with_capacity(INIT_LEN),
            properties: Properties {
                span_ids: Vec::with_capacity(INIT_LEN),
                property_lens: Vec::with_capacity(INIT_LEN),
                payload: Vec::with_capacity(INIT_BYTES_LEN),
            },
        }
    }

    // pub fn from_span(span: Span) -> Self {
    //     let mut span_set = SpanSet {
    //         spans: Vec::with_capacity(1),
    //         properties: Properties {
    //             span_ids: Vec::with_capacity(0),
    //             property_lens: Vec::with_capacity(0),
    //             payload: Vec::with_capacity(0),
    //         },
    //     };
    //     span_set.spans.push(span);
    //     span_set
    // }

    // pub fn is_empty(&self) -> Self {
    //     self.spans.is_empty() &&
    //     self.properties.span_ids.is_empty() &&
    //     self.properties.property_lens.is_empty() &&
    //     self.properties.payload.is_empty()
    // }

    pub fn take(&mut self) -> Self {
        SpanSet {
            spans: self.spans.split_off(0),
            properties: Properties {
                span_ids: self.properties.span_ids.split_off(0),
                property_lens: self.properties.property_lens.split_off(0),
                payload: self.properties.payload.split_off(0),
            },
        }
    }

    pub fn append(&mut self, mut other: Self) {
        self.spans.append(&mut other.spans);
        self.properties
            .span_ids
            .append(&mut other.properties.span_ids);
        self.properties
            .property_lens
            .append(&mut other.properties.property_lens);
        self.properties
            .payload
            .append(&mut other.properties.payload);
    }
}
