// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::span::cycle::DefaultClock;
use crate::span::span_id::SpanId;
use crate::span::{ScopeSpan, Span};
use crossbeam_channel::Sender;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum SpanCollection {
    LocalSpans {
        spans: VecDeque<Span>,
        parent_span_id: SpanId,
    },
    ScopeSpan(Span),
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

    pub fn combine<'a, I: Iterator<Item = &'a AcquirerGroup>>(
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

    pub fn submit(&self, spans: VecDeque<Span>) {
        self.submit_to_acquirers(SpanCollection::LocalSpans {
            spans,
            parent_span_id: self.scope_span.id,
        });
    }

    pub fn submit_scope_span(&self, scope_span: Span) {
        self.submit_to_acquirers(SpanCollection::ScopeSpan(scope_span));
    }
}

impl AcquirerGroup {
    fn submit_to_acquirers(&self, span_collection: SpanCollection) {
        // save one clone
        for acq in self.acquirers.iter().skip(1) {
            acq.submit(span_collection.clone());
        }
        if let Some(acq) = self.acquirers.first() {
            acq.submit(span_collection);
        }
    }
}

impl Drop for AcquirerGroup {
    fn drop(&mut self) {
        self.submit_scope_span(self.scope_span.to_span(DefaultClock::now()));
    }
}
