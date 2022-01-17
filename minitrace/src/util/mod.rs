// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

pub mod guard;
pub mod object_pool;
pub mod spsc;

use std::cell::RefCell;

use once_cell::sync::Lazy;

use crate::collector::ParentSpan;
use crate::local::raw_span::RawSpan;
use crate::util::object_pool::{Pool, Puller, Reusable};

static RAW_SPANS_POOL: Lazy<Pool<Vec<RawSpan>>> = Lazy::new(|| Pool::new(Vec::new, Vec::clear));
static PARENT_SPANS_POOL: Lazy<Pool<Vec<ParentSpan>>> =
    Lazy::new(|| Pool::new(Vec::new, Vec::clear));

thread_local! {
    static RAW_SPANS_PULLER: RefCell<Puller<'static, Vec<RawSpan>>> = RefCell::new(RAW_SPANS_POOL.puller(512));
    static PARENT_SPANS_PULLER: RefCell<Puller<'static, Vec<ParentSpan>>>  = RefCell::new(PARENT_SPANS_POOL.puller(512));
}

pub type RawSpans = Reusable<'static, Vec<RawSpan>>;
pub type ParentSpans = Reusable<'static, Vec<ParentSpan>>;

pub(crate) fn alloc_raw_spans() -> RawSpans {
    RAW_SPANS_PULLER.with(|puller| puller.borrow_mut().pull())
}

pub(crate) fn alloc_parent_spans() -> ParentSpans {
    PARENT_SPANS_PULLER.with(|puller| puller.borrow_mut().pull())
}
