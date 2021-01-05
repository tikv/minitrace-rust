// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::local::observer::{Observer, RawSpans};
use crate::local::scope_guard::{LocalScope, LocalScopeGuard};
use crate::span::cycle::{Cycle, DefaultClock};
use crate::span::span_id::{DefaultIdGenerator, SpanId};
use crate::span::RawSpan;
use crate::trace::acquirer::{Acquirer, SpanCollection};
use crate::Collector;

#[derive(Debug)]
pub struct Scope {
    inner: Option<ScopeInner>,
}

#[derive(Debug)]
struct ScopeInner {
    scope_id: SpanId,
    event: &'static str,
    begin_cycle: Cycle,

    /// [parent scope id -> acquirer]
    collectors: Vec<(SpanId, Acquirer)>,
}

impl Scope {
    pub fn root(event: &'static str) -> (Self, Collector) {
        let (tx, rx) = crossbeam::channel::unbounded();
        let closed = Arc::new(AtomicBool::new(false));
        let inner = ScopeInner {
            scope_id: DefaultIdGenerator::next_id(),
            event,
            begin_cycle: DefaultClock::now(),
            collectors: vec![(SpanId::new(0), Acquirer::new(Arc::new(tx), closed.clone()))],
        };
        let scope = Self { inner: Some(inner) };
        let collector = Collector::new(rx, closed);
        (scope, collector)
    }

    pub fn child(&self, event: &'static str) -> Self {
        if let Some(inner) = &self.inner {
            let child_inner = ScopeInner {
                scope_id: DefaultIdGenerator::next_id(),
                event,
                begin_cycle: DefaultClock::now(),
                collectors: inner
                    .collectors
                    .iter()
                    .map(|(_, acq)| (inner.scope_id, acq.clone()))
                    .collect(),
            };

            Self {
                inner: Some(child_inner),
            }
        } else {
            Self { inner: None }
        }
    }

    #[inline]
    pub fn empty() -> Self {
        Self { inner: None }
    }

    pub fn merge<'a>(scopes: impl Iterator<Item = &'a Scope>, event: &'static str) -> Self {
        let mut collectors = Vec::new();
        for scope in scopes {
            if let Some(inner) = &scope.inner {
                let scope_id = inner.scope_id;
                for (_, acq) in &inner.collectors {
                    collectors.push((scope_id, acq.clone()))
                }
            }
        }

        if collectors.is_empty() {
            Self { inner: None }
        } else {
            Self {
                inner: Some(ScopeInner {
                    scope_id: DefaultIdGenerator::next_id(),
                    event,
                    begin_cycle: DefaultClock::now(),
                    collectors,
                }),
            }
        }
    }

    #[inline]
    pub fn submit_raw_spans(&self, raw_spans: Arc<RawSpans>) {
        if let Some(inner) = &self.inner {
            for (_, acq) in &inner.collectors {
                acq.submit(SpanCollection::RawSpans {
                    raw_spans: raw_spans.clone(),
                    scope_id: inner.scope_id,
                })
            }
        }
    }

    #[inline]
    pub fn attach(self) -> LocalScopeGuard {
        LocalScopeGuard::new(self)
    }

    #[inline]
    pub fn attach_and_observe(self) -> LocalScopeGuard {
        LocalScopeGuard::new_with_observer(self, Observer::attach())
    }

    #[inline]
    pub fn child_from_local(event: &'static str) -> Self {
        LocalScope::with_local_scope(|s| {
            if let Some(scope) = s {
                scope.child(event)
            } else {
                Self::empty()
            }
        })
    }
}

impl Drop for ScopeInner {
    fn drop(&mut self) {
        for (parent_id, collector) in &self.collectors {
            collector.submit(SpanCollection::ScopeSpan(RawSpan {
                id: self.scope_id,
                parent_id: *parent_id,
                begin_cycle: self.begin_cycle,
                event: self.event,
                properties: vec![],
                end_cycle: DefaultClock::now(),
            }))
        }
    }
}
