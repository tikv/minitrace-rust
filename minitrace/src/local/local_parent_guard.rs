// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::local_collector::LocalCollector;
use crate::span::Span;
use crate::util::alloc_parent_spans;

#[must_use]
pub struct LocalParentGuard {
    _local_collector: Option<LocalCollector>,
}

impl LocalParentGuard {
    pub(crate) fn new(span: &Span) -> Self {
        let local_collector = span.inner.as_ref().map(|inner| {
            let mut parents = alloc_parent_spans();
            parents.extend(inner.as_parent());
            LocalCollector::start_with_parent(parents)
        });

        Self {
            _local_collector: local_collector,
        }
    }
}
