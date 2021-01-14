// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;
use std::sync::Arc;

use crate::local::local_collector::LocalCollector;
use crate::span::span_id::SpanId;
use crate::trace::acquirer::{Acquirer, SpanCollection};
use crate::Scope;

thread_local! {
    static LOCAL_SCOPE: RefCell<Option<LocalScope>> = RefCell::new(None);
}

pub struct LocalScope {
    scope_id: SpanId,
    acquirers: Vec<Acquirer>,

    local_collector: Option<LocalCollector>,
}

impl LocalScope {
    pub fn new_child_scope(event: &'static str) -> Scope {
        LOCAL_SCOPE.with(|local_scope| {
            let local_scope = local_scope.borrow();
            if let Some(LocalScope {
                scope_id: parent_scope_id,
                acquirers,
                ..
            }) = local_scope.as_ref()
            {
                Scope::new(acquirers.iter().map(|acq| (*parent_scope_id, acq)), event)
            } else {
                Scope::empty()
            }
        })
    }

    #[inline]
    pub fn is_occupied() -> bool {
        LOCAL_SCOPE.with(|local_scope| {
            let local_scope = local_scope.borrow();
            local_scope.is_some()
        })
    }
}

pub struct LocalScopeGuard;
impl !Send for LocalScopeGuard {}
impl !Sync for LocalScopeGuard {}

impl Drop for LocalScopeGuard {
    fn drop(&mut self) {
        LOCAL_SCOPE.with(|local_scope| {
            if let Some(LocalScope {
                scope_id,
                acquirers,
                local_collector: Some(local_collector),
            }) = local_scope.borrow_mut().take()
            {
                let raw_spans = Arc::new(local_collector.collect());
                for acq in acquirers {
                    acq.submit(SpanCollection::RawSpans {
                        raw_spans: raw_spans.clone(),
                        scope_id,
                    })
                }
            }
        })
    }
}

impl LocalScopeGuard {
    #[inline]
    pub(crate) fn new_with_local_collector(
        scope: &Scope,
        local_collector: Option<LocalCollector>,
    ) -> Self {
        LOCAL_SCOPE.with(|local_scope| {
            let mut local_scope = local_scope.borrow_mut();

            if local_scope.is_some() {
                panic!("Attach too much scopes: > 1")
            }

            if let Some(inner) = &scope.inner {
                *local_scope = Some(LocalScope {
                    scope_id: inner.scope_id,
                    acquirers: inner.to_report.iter().map(|(_, acq)| acq.clone()).collect(),
                    local_collector,
                })
            }
        });

        LocalScopeGuard
    }
}

impl Scope {
    #[inline]
    pub fn enter(&self) -> LocalScopeGuard {
        self.try_enter()
            .expect("Current thread is occupied by another scope")
    }

    #[inline]
    pub fn try_enter(&self) -> Option<LocalScopeGuard> {
        if LocalScope::is_occupied() {
            None
        } else {
            Some(LocalScopeGuard::new_with_local_collector(
                self,
                LocalCollector::try_start(),
            ))
        }
    }

    #[inline]
    pub fn from_local_parent(event: &'static str) -> Self {
        LocalScope::new_child_scope(event)
    }
}
