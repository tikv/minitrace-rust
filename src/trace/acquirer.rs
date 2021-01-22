// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam::channel::Sender;

use crate::local::local_collector::LocalSpans;
use crate::span::{RawSpan, SpanId};

#[derive(Clone, Debug)]
pub enum SpanCollection {
    LocalSpans {
        local_spans: Arc<LocalSpans>,
        parent_id_of_root: SpanId,
    },
    Span(RawSpan),
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
