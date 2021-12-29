// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;

use minstant::Cycle;

use crate::local::local_span_line::LOCAL_SPAN_LINE;
use crate::local::raw_span::RawSpan;

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

#[derive(Debug)]
pub struct LocalSpans {
    pub(crate) spans: Vec<RawSpan>,
    pub(crate) end_time: Cycle,
}

impl LocalCollector {
    pub(crate) fn new(local_collector_epoch: usize) -> Self {
        Self {
            collected: false,
            local_collector_epoch,
            _p: Default::default(),
        }
    }

    pub fn start() -> Option<Self> {
        let collector = LOCAL_SPAN_LINE.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            s.register_local_collector()
        });
        debug_assert!(
            collector.is_some(),
            "Current thread is occupied by another local collector"
        );
        collector
    }

    pub fn collect(mut self) -> LocalSpans {
        LOCAL_SPAN_LINE.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            self.collected = true;
            LocalSpans {
                spans: s.unregister_and_collect(self),
                end_time: Cycle::now(),
            }
        })
    }
}

impl Drop for LocalCollector {
    fn drop(&mut self) {
        if !self.collected {
            self.collected = true;
            LOCAL_SPAN_LINE.with(|span_line| {
                let s = &mut *span_line.borrow_mut();
                s.clear();
            })
        }
    }
}
