// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use smallvec::SmallVec;

use crate::collector::acquirer::Acquirer;
use crate::local::local_collector::LocalCollector;
use crate::local::span_id::SpanId;
use crate::span::Span;

pub(crate) struct LocalParentSpan {
    pub(crate) span_id: SpanId,
    pub(crate) acquirers: SmallVec<[Acquirer; 1]>,
}

#[must_use]
pub struct LocalParentGuard {
    _local_collector: Option<LocalCollector>,
}

impl LocalParentGuard {
    pub(crate) fn new(span: &Span) -> Self {
        if let Some(inner) = &span.inner {
            let local_parent = LocalParentSpan {
                span_id: inner.span_id,
                acquirers: inner.to_report.iter().map(|(_, acq)| acq.clone()).collect(),
            };
            Self {
                _local_collector: Some(LocalCollector::start_with_parent(local_parent)),
            }
        } else {
            Self {
                _local_collector: None,
            }
        }
    }
}
