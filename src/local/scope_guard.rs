// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use smallvec::SmallVec;
use std::sync::Arc;

use crate::local::registry::Listener;
use crate::local::span_line::SPAN_LINE;
use crate::trace::acquirer::AcquirerGroup;
use crate::Scope;

pub struct LocalScopeGuard {
    listener: Option<Listener>,
}

impl !Sync for LocalScopeGuard {}
impl !Send for LocalScopeGuard {}

impl LocalScopeGuard {
    pub(crate) fn new(acquirer_group: Option<Arc<AcquirerGroup>>) -> Self {
        SPAN_LINE.with(|span_line| {
            let mut span_line = span_line.borrow_mut();
            Self {
                listener: acquirer_group
                    .map(|acq_group| span_line.register(smallvec::smallvec![acq_group])),
            }
        })
    }

    pub fn new_from_scopes<'a, I: Iterator<Item = &'a Scope>>(iter: I) -> Self {
        use std::iter::FromIterator;

        SPAN_LINE.with(|span_line| {
            let mut span_line = span_line.borrow_mut();
            let sv = SmallVec::from_iter(iter.filter_map(|scope| scope.acquirer_group.clone()));
            Self {
                listener: if sv.is_empty() {
                    None
                } else {
                    Some(span_line.register(sv))
                },
            }
        })
    }
}

impl Drop for LocalScopeGuard {
    fn drop(&mut self) {
        if let Some(listener) = self.listener {
            SPAN_LINE.with(|span_line| {
                let mut span_line = span_line.borrow_mut();
                let (acgs, spans) = span_line.unregister_and_collect(listener);
                let spans = Arc::new(spans);
                if !spans.is_empty() {
                    for acg in acgs {
                        acg.submit(spans.clone());
                    }
                }
            })
        }
    }
}
