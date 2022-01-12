// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::local_collector::LocalCollector;
use crate::span::Span;

#[must_use]
pub struct LocalParentGuard {
    _local_collector: Option<LocalCollector>,
}

impl LocalParentGuard {
    pub(crate) fn new(span: &Span) -> Self {
        let local_collector = span
            .inner
            .as_ref()
            .map(|inner| LocalCollector::start_with_parent(inner.as_parent().collect()));

        Self {
            _local_collector: local_collector,
        }
    }
}
