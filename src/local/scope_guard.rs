// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::registry::Listener;
use crate::local::span_line::SPAN_LINE;
use crate::trace::acquirer::AcquirerGroup;
use std::sync::Arc;

pub struct LocalScopeGuard {
    listener: Option<Listener>,
}

impl !Sync for LocalScopeGuard {}
impl !Send for LocalScopeGuard {}

impl LocalScopeGuard {
    pub fn new(acquirer_group: Option<Arc<AcquirerGroup>>) -> Self {
        SPAN_LINE.with(|span_line| {
            let mut span_line = span_line.borrow_mut();
            Self {
                listener: acquirer_group.map(|acq_group| span_line.register_now(acq_group)),
            }
        })
    }
}

impl Drop for LocalScopeGuard {
    fn drop(&mut self) {
        if let Some(listener) = self.listener {
            SPAN_LINE.with(|span_line| {
                let mut span_line = span_line.borrow_mut();
                let (acg, spans) = span_line.unregister_and_collect(listener);
                acg.submit(spans);
            })
        }
    }
}
