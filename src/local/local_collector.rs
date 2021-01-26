// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;

use crate::local::span_line::SPAN_LINE;
use crate::span::RawSpan;
use crate::span::{Cycle, DefaultClock};

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
    // TODO: replace it until feature `negative_impls` is stable.
    _p: PhantomData<*const ()>,
}

#[derive(Debug)]
pub struct LocalSpans {
    pub spans: Vec<RawSpan>,
    pub end_time: Cycle,
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
        Self::try_start().expect("Current thread is occupied by another local collector")
    }

    pub fn try_start() -> Option<Self> {
        SPAN_LINE.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            s.register_local_collector()
        })
    }

    pub fn collect(mut self) -> LocalSpans {
        SPAN_LINE.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            self.collected = true;
            LocalSpans {
                spans: s.unregister_and_collect(self),
                end_time: DefaultClock::now(),
            }
        })
    }
}

impl Drop for LocalCollector {
    fn drop(&mut self) {
        if !self.collected {
            self.collected = true;
            SPAN_LINE.with(|span_line| {
                let s = &mut *span_line.borrow_mut();
                s.clear();
            })
        }
    }
}
