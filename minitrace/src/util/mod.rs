// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

pub mod object_pool;
pub mod spsc;

use std::cell::RefCell;

use once_cell::sync::Lazy;

use crate::local::raw_span::RawSpan;
use crate::util::object_pool::{Pool, Puller, Reusable};

static RAW_SPANS_POOL: Lazy<Pool<Vec<RawSpan>>> = Lazy::new(|| Pool::new(Vec::new, Vec::clear));

thread_local! {
    static RAW_SPANS_PULLER: RefCell<Puller< 'static,Vec<RawSpan>>>  = RefCell::new(RAW_SPANS_POOL.puller(128));
}

pub(crate) type RawSpans = Reusable<'static, Vec<RawSpan>>;

pub(crate) fn alloc_raw_spans() -> RawSpans {
    RAW_SPANS_PULLER.with(|puller| puller.borrow_mut().pull())
}
