// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::iter;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::local::observer::{Observer, RawSpans};
use crate::local::scope_guard::{LocalScope, LocalScopeGuard};
use crate::span::cycle::DefaultClock;
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

    // Report `RawSpan` to `Acquirer` when `ScopeInner` is dropping
    to_report: Vec<(RawSpan, Acquirer)>,
}

impl Scope {
    pub fn root(event: &'static str) -> (Self, Collector) {
        let (tx, rx) = crossbeam::channel::unbounded();
        let closed = Arc::new(AtomicBool::new(false));
        let scope_id = DefaultIdGenerator::next_id();
        let inner = ScopeInner {
            scope_id,
            to_report: vec![(
                RawSpan::begin_with(scope_id, SpanId::new(0), DefaultClock::now(), event),
                Acquirer::new(Arc::new(tx), closed.clone()),
            )],
        };
        let scope = Self { inner: Some(inner) };
        let collector = Collector::new(rx, closed);
        (scope, collector)
    }

    pub fn child(&self, event: &'static str) -> Self {
        Self::merge(iter::once(self), event)
    }

    #[inline]
    pub fn empty() -> Self {
        Self { inner: None }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_none()
    }

    pub fn merge<'a>(scopes: impl Iterator<Item = &'a Scope>, event: &'static str) -> Self {
        let mut to_report = Vec::new();
        let now = DefaultClock::now();
        let scope_id = DefaultIdGenerator::next_id();
        for scope in scopes {
            if let Some(inner) = &scope.inner {
                let parent_id = inner.scope_id;
                for (_, acq) in &inner.to_report {
                    to_report.push((
                        RawSpan::begin_with(scope_id, parent_id, now, event),
                        acq.clone(),
                    ))
                }
            }
        }

        if to_report.is_empty() {
            Self { inner: None }
        } else {
            Self {
                inner: Some(ScopeInner {
                    scope_id,
                    to_report,
                }),
            }
        }
    }

    #[inline]
    pub fn submit_raw_spans(&self, raw_spans: Arc<RawSpans>) {
        if let Some(inner) = &self.inner {
            for (_, acq) in &inner.to_report {
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
    pub fn try_attach(self) -> std::result::Result<LocalScopeGuard, Self> {
        if LocalScope::is_occupied() {
            Err(self)
        } else {
            Ok(LocalScopeGuard::new(self))
        }
    }

    #[inline]
    pub fn attach_and_observe(self) -> LocalScopeGuard {
        LocalScopeGuard::new_with_observer(self, Observer::attach())
    }

    #[inline]
    pub fn try_attach_and_observe(self) -> std::result::Result<LocalScopeGuard, Self> {
        if LocalScope::is_occupied() {
            Err(self)
        } else {
            Ok(LocalScopeGuard::new_with_observer(self, Observer::attach()))
        }
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
        let now = DefaultClock::now();
        for (mut span, collector) in self.to_report.drain(..) {
            span.end_with(now);
            collector.submit(SpanCollection::ScopeSpan(span))
        }
    }
}
