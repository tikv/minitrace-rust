// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;
use std::sync::Arc;

use crate::local::observer::Observer;
use crate::span::span_id::SpanId;
use crate::trace::acquirer::{Acquirer, SpanCollection};
use crate::Scope;

thread_local! {
    static LOCAL_SCOPE: RefCell<Option<LocalScope>> = RefCell::new(None);
}

pub struct LocalScope {
    scope_id: SpanId,
    acquirers: Vec<Acquirer>,

    observer: Option<Observer>,
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
                observer: Some(observer),
            }) = local_scope.borrow_mut().take()
            {
                let raw_spans = Arc::new(observer.collect());
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
    pub fn new(scope: &Scope) -> Self {
        Self::new_with_observer(scope, None)
    }

    #[inline]
    pub fn new_with_observer(scope: &Scope, observer: Option<Observer>) -> Self {
        LOCAL_SCOPE.with(|local_scope| {
            let mut local_scope = local_scope.borrow_mut();

            if local_scope.is_some() {
                panic!("Attach too much scopes: > 1")
            }

            if let Some(inner) = &scope.inner {
                *local_scope = Some(LocalScope {
                    scope_id: inner.scope_id,
                    acquirers: inner.to_report.iter().map(|(_, acq)| acq.clone()).collect(),
                    observer,
                })
            }
        });

        LocalScopeGuard
    }
}
