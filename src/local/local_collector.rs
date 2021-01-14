// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::span_line::SPAN_LINE;
use crate::span::cycle::{Cycle, DefaultClock};
use crate::span::RawSpan;

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct LocalCollector {
    pub(crate) local_collector_epoch: usize,
}
impl !Sync for LocalCollector {}
impl !Send for LocalCollector {}

#[derive(Debug)]
pub struct RawSpans {
    pub spans: Vec<RawSpan>,
    pub end_time: Cycle,
}

impl LocalCollector {
    pub fn start() -> Self {
        Self::try_start().expect("Current thread is occupied by another local collector")
    }

    pub fn try_start() -> Option<Self> {
        SPAN_LINE.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            s.register_local_collector()
        })
    }

    pub fn collect(self) -> RawSpans {
        SPAN_LINE.with(|span_line| {
            let s = &mut *span_line.borrow_mut();
            RawSpans {
                spans: s.unregister_and_collect(self),
                end_time: DefaultClock::now(),
            }
        })
    }
}
