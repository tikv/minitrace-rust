// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use slab::Slab;
use smallvec::SmallVec;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;

use crate::local::registry::{Listener, Registry};
use crate::span::span_queue::{SpanHandle, SpanQueue};
use crate::span::{ScopeSpan, Span};
use crate::trace::acquirer::AcquirerGroup;

thread_local! {
    pub(super) static SPAN_LINE: RefCell<SpanLine> = RefCell::new(SpanLine::new());
}

pub struct SpanLine {
    span_queue: SpanQueue,
    registry: Registry,
    local_acquirer_groups: Slab<SmallVec<[Arc<AcquirerGroup>; 4]>>,
}

impl SpanLine {
    pub fn new() -> Self {
        Self {
            span_queue: SpanQueue::new(),
            registry: Registry::default(),
            local_acquirer_groups: Slab::default(),
        }
    }

    #[inline]
    pub fn start_span(&mut self, event: &'static str) -> Option<SpanHandle> {
        if self.registry.is_empty() {
            return None;
        }

        Some(self.span_queue.start_span(event))
    }

    #[inline]
    pub fn finish_span(&mut self, span_handle: SpanHandle) {
        self.span_queue.finish_span(span_handle);
    }

    pub fn register(&mut self, acquirer_group: SmallVec<[Arc<AcquirerGroup>; 4]>) -> Listener {
        debug_assert_eq!(
            self.local_acquirer_groups.len(),
            self.registry.len(),
            "expect same length, but length of local_acquirer_groups is {}, length of registry is {}",
            self.local_acquirer_groups.len(),
            self.registry.len(),
        );

        let slab_idx = self.local_acquirer_groups.insert(acquirer_group);
        let l = Listener::new(self.span_queue.next_index(), slab_idx);
        self.registry.register(l);
        l
    }

    pub fn unregister_and_collect(
        &mut self,
        listener: Listener,
    ) -> (SmallVec<[Arc<AcquirerGroup>; 4]>, VecDeque<Span>) {
        debug_assert_eq!(
            self.local_acquirer_groups.len(),
            self.registry.len(),
            "expect same length, but length of local_acquirer_groups is {}, length of registry is {}",
            self.local_acquirer_groups.len(),
            self.registry.len(),
        );

        let acg = self.local_acquirer_groups.remove(listener.slab_index);
        self.registry.unregister(listener);

        let spans = if self.registry.is_empty() {
            self.span_queue.take_queue_from(listener.queue_index)
        } else {
            let s = self.span_queue.clone_queue_from(listener.queue_index);
            self.gc();
            s
        };

        (acg, spans)
    }

    /// Return `None` if there're no registered acquirers, or all acquirers
    /// combined into one group.
    pub fn merge_registered_acquirers(&mut self, event: &'static str) -> Option<AcquirerGroup> {
        match self.start_scope_span("<spawn>", event) {
            None => None,
            Some(ss) => AcquirerGroup::combine(
                self.local_acquirer_groups
                    .iter()
                    .flat_map(|s| s.1.iter().map(|acg| acg.as_ref())),
                ss,
            ),
        }
    }

    #[inline]
    pub fn add_properties<I: IntoIterator<Item = (&'static str, String)>, F: FnOnce() -> I>(
        &mut self,
        span_handle: &SpanHandle,
        properties: F,
    ) {
        self.span_queue.add_properties(span_handle, properties);
    }

    #[inline]
    pub fn add_property<F: FnOnce() -> (&'static str, String)>(
        &mut self,
        span_handle: &SpanHandle,
        property: F,
    ) {
        self.span_queue.add_property(span_handle, property);
    }
}

impl SpanLine {
    fn gc(&mut self) {
        if let Some(l) = self.registry.oldest_listener() {
            self.span_queue.remove_before(l.queue_index);
        }
    }

    fn start_scope_span(
        &mut self,
        placeholder_event: &'static str,
        event: &'static str,
    ) -> Option<ScopeSpan> {
        if self.registry.is_empty() {
            return None;
        }

        Some(self.span_queue.start_scope_span(placeholder_event, event))
    }
}
