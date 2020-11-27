// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crossbeam_channel::Sender;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::span::cycle::DefaultClock;
use crate::span::span_id::SpanId;
use crate::span::{RawSpan, ScopeSpan};

#[derive(Clone, Debug)]
pub enum SpanCollection {
    LocalSpans {
        spans: Arc<VecDeque<RawSpan>>,
        parent_span_id: SpanId,
    },
    ScopeSpan(ScopeSpan),
}

#[derive(Clone, Debug)]
pub struct Acquirer {
    sender: Arc<Sender<SpanCollection>>,
    closed: Arc<AtomicBool>,
}

impl Acquirer {
    pub fn new(sender: Arc<Sender<SpanCollection>>, closed: Arc<AtomicBool>) -> Self {
        Acquirer { sender, closed }
    }

    pub fn submit(&self, span_collection: SpanCollection) {
        if self.is_shutdown() {
            return;
        }

        self.sender.send(span_collection).ok();
    }

    pub fn is_shutdown(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug)]
pub struct AcquirerGroup {
    /// A span represents task processing
    scope_span: ScopeSpan,
    acquirers: Vec<Acquirer>,
}

impl AcquirerGroup {
    pub fn new(span: ScopeSpan, acquirers: Vec<Acquirer>) -> Self {
        debug_assert!(!acquirers.is_empty());

        AcquirerGroup {
            scope_span: span,
            acquirers,
        }
    }

    pub fn combine<'a, I: Iterator<Item = &'a Self>>(
        iter: I,
        scope_span: ScopeSpan,
    ) -> Option<Self> {
        let acquirers = iter
            .map(|s| {
                s.acquirers.iter().filter_map(|acq| {
                    if acq.is_shutdown() {
                        None
                    } else {
                        Some(acq.clone())
                    }
                })
            })
            .flatten()
            .collect::<Vec<_>>();

        if acquirers.is_empty() {
            None
        } else {
            Some(Self {
                scope_span,
                acquirers,
            })
        }
    }

    pub fn submit(&self, spans: Arc<VecDeque<RawSpan>>) {
        self.submit_to_acquirers(SpanCollection::LocalSpans {
            spans,
            parent_span_id: self.scope_span.id,
        });
    }

    pub fn submit_scope_span(&self, scope_span: ScopeSpan) {
        self.submit_to_acquirers(SpanCollection::ScopeSpan(scope_span));
    }
}

impl AcquirerGroup {
    fn submit_to_acquirers(&self, span_collection: SpanCollection) {
        for acq in &self.acquirers {
            acq.submit(span_collection.clone());
        }
    }
}

impl Drop for AcquirerGroup {
    fn drop(&mut self) {
        self.scope_span.end_cycle = DefaultClock::now();
        self.submit_scope_span(self.scope_span);
    }
}
