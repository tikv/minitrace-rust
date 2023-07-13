// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

pub mod object_pool;
pub mod spsc;
#[doc(hidden)]
pub mod tree;

use std::cell::RefCell;
use std::iter::FromIterator;

use once_cell::sync::Lazy;

use crate::collector::CollectTokenItem;
use crate::local::raw_span::RawSpan;
use crate::util::object_pool::Pool;
use crate::util::object_pool::Puller;
use crate::util::object_pool::Reusable;

static RAW_SPANS_POOL: Lazy<Pool<Vec<RawSpan>>> = Lazy::new(|| Pool::new(Vec::new, Vec::clear));
static COLLECT_TOKEN_ITEMS_POOL: Lazy<Pool<Vec<CollectTokenItem>>> =
    Lazy::new(|| Pool::new(Vec::new, Vec::clear));

thread_local! {
    static RAW_SPANS_PULLER: RefCell<Puller<'static, Vec<RawSpan>>> = RefCell::new(RAW_SPANS_POOL.puller(512));
    static COLLECT_TOKEN_ITEMS_PULLER: RefCell<Puller<'static, Vec<CollectTokenItem>>>  = RefCell::new(COLLECT_TOKEN_ITEMS_POOL.puller(512));
}

pub type RawSpans = Reusable<'static, Vec<RawSpan>>;
pub type CollectToken = Reusable<'static, Vec<CollectTokenItem>>;

impl Default for RawSpans {
    fn default() -> Self {
        RAW_SPANS_PULLER.with(|puller| puller.borrow_mut().pull())
    }
}

fn new_collect_token(items: impl IntoIterator<Item = CollectTokenItem>) -> CollectToken {
    let mut token = COLLECT_TOKEN_ITEMS_PULLER.with(|puller| puller.borrow_mut().pull());
    token.extend(items);
    token
}

impl FromIterator<CollectTokenItem> for CollectToken {
    fn from_iter<T: IntoIterator<Item = CollectTokenItem>>(iter: T) -> Self {
        new_collect_token(iter)
    }
}

impl<'a> FromIterator<&'a CollectTokenItem> for CollectToken {
    fn from_iter<T: IntoIterator<Item = &'a CollectTokenItem>>(iter: T) -> Self {
        new_collect_token(iter.into_iter().copied())
    }
}

impl From<CollectTokenItem> for CollectToken {
    fn from(item: CollectTokenItem) -> Self {
        new_collect_token([item])
    }
}
