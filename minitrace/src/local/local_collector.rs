// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;

use minstant::Instant;

use crate::collector::RawSpans;
use crate::local::local_parent_guard::LocalParentSpan;
use crate::local::local_span_line::LOCAL_SPAN_STACK;

#[must_use]
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct LocalCollector {
    pub(crate) collected: bool,
    pub(crate) local_collector_epoch: usize,

    // Identical to
    // ```
    // impl !Sync for LocalCollector {}
    // impl !Send for LocalCollector {}
    // ```
    //
    // TODO: Replace it once feature `negative_impls` is stable.
    _p: PhantomData<*const ()>,
}

pub struct LocalSpans {
    pub(crate) spans: RawSpans,
    pub(crate) end_time: Instant,
}

impl LocalCollector {
    pub(crate) fn new(local_collector_epoch: usize) -> Self {
        Self {
            collected: false,
            local_collector_epoch,
            _p: Default::default(),
        }
    }

    pub fn start() -> Self {
        LOCAL_SPAN_STACK.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            s.register_local_collector(None)
        })
    }

    pub(crate) fn start_with_parent(parent: LocalParentSpan) -> Self {
        LOCAL_SPAN_STACK.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            s.register_local_collector(Some(parent))
        })
    }

    pub fn collect(mut self) -> LocalSpans {
        LOCAL_SPAN_STACK.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            self.collected = true;
            LocalSpans {
                // This will panic if `LocalCollector` is started by `start_with_parent_span`
                spans: s.unregister_and_collect(&self).unwrap(),
                end_time: Instant::now(),
            }
        })
    }
}

impl Drop for LocalCollector {
    fn drop(&mut self) {
        if !self.collected {
            self.collected = true;
            LOCAL_SPAN_STACK.with(|span_line| {
                let s = &mut *span_line.borrow_mut();
                s.unregister_and_collect(self);
            })
        }
    }
}
