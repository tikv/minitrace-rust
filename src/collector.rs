// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crossbeam::channel::Receiver;

use crate::trace::Span;
use crate::utils::real_time_ns;

const INIT_LEN: usize = 1024;
const INIT_BYTES_LEN: usize = 16384;

pub struct Collector {
    rx: Receiver<SpanSet>,
    trace_id: u64,
    start_time_ns: u64,
    start_time_cycles: u64,
}

impl Collector {
    pub(crate) fn new(trace_id: u64, rx: Receiver<SpanSet>) -> Self {
        Collector {
            rx,
            trace_id,
            start_time_ns: real_time_ns(),
            start_time_cycles: minstant::now(),
        }
    }

    pub fn finish(self) -> TraceResult {
        let mut span_set = SpanSet::new();
        let elapsed_ns = real_time_ns() - self.start_time_ns;

        for other_span_set in self.rx.try_iter() {
            span_set.extend_from(&other_span_set);
        }

        TraceResult {
            trace_id: self.trace_id,
            start_time_ns: self.start_time_ns,
            start_time_cycles: self.start_time_cycles,
            elapsed_ns,
            cycles_per_second: minstant::cycles_per_second(),
            spans: span_set.spans,
            properties: span_set.properties,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraceResult {
    pub trace_id: u64,

    pub start_time_ns: u64,
    pub start_time_cycles: u64,
    pub elapsed_ns: u64,

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

    pub fn extend_from(&mut self, other: &Self) {
        self.spans.extend_from_slice(&other.spans);
        self.properties
            .span_ids
            .extend_from_slice(&other.properties.span_ids);
        self.properties
            .property_lens
            .extend_from_slice(&other.properties.property_lens);
        self.properties
            .payload
            .extend_from_slice(&other.properties.payload);
    }
}
