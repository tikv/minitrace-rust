// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::global_collector::Global;
use crate::collector::Collect;
use crate::local::local_collector::LocalCollector;
use crate::local::local_span_line::LOCAL_SPAN_STACK;
use crate::util::alloc_parent_spans;
use crate::Span;

use std::rc::Rc;

#[must_use]
pub struct LocalParentGuard<C: Collect = Global> {
    _local_collector: Option<LocalCollector<C>>,
}

impl<C: Collect> LocalParentGuard<C> {
    pub(crate) fn new(span: &Span<C>, collect: C) -> Self {
        let local_collector = span.inner.as_ref().map(|inner| {
            let mut parents = alloc_parent_spans();
            parents.extend(inner.as_parent());

            let stack = LOCAL_SPAN_STACK.with(Rc::clone);
            LocalCollector::new(stack, Some(parents), collect)
        });

        Self {
            _local_collector: local_collector,
        }
    }
}
