// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::global_collector::Global;
use crate::collector::Collect;
use crate::local::local_collector::LocalCollector;
use crate::util::alloc_parent_spans;
use crate::Span;

#[must_use]
pub struct LocalParentGuard<C: Collect = Global> {
    _local_collector: Option<LocalCollector<C>>,
}

impl<C: Collect> LocalParentGuard<C> {
    pub(crate) fn new(span: &Span<C>) -> Self {
        let local_collector = span.inner.as_ref().map(|inner| {
            let mut parents = alloc_parent_spans();
            parents.extend(inner.as_parent());
            LocalCollector::<C>::start_with_parent(parents)
        });

        Self {
            _local_collector: local_collector,
        }
    }
}
