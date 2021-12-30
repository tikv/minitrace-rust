// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;

use crate::local::local_span_line::{LocalSpanHandle, LOCAL_SPAN_STACK};

#[must_use]
pub struct LocalSpanGuard {
    span_handle: Option<LocalSpanHandle>,

    // Identical to
    // ```
    // impl !Sync for LocalSpanGuard {}
    // impl !Send for LocalSpanGuard {}
    // ```
    //
    // TODO: Replace it once feature `negative_impls` is stable.
    _p: PhantomData<*const ()>,
}

impl LocalSpanGuard {
    #[inline]
    pub(crate) fn new(event: &'static str) -> Self {
        LOCAL_SPAN_STACK.with(|span_line| {
            let mut span_line = span_line.borrow_mut();
            let span_handle = span_line.enter_span(event);
            Self {
                span_handle,
                _p: Default::default(),
            }
        })
    }

    #[inline]
    pub fn with_property<F>(self, property: F) -> Self
    where
        F: FnOnce() -> (&'static str, String),
    {
        self.with_properties(|| [property()])
    }

    #[inline]
    pub fn with_properties<I, F>(self, properties: F) -> Self
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        if let Some(local_span_handle) = &self.span_handle {
            LOCAL_SPAN_STACK.with(|span_stack| {
                let span_stack = &mut *span_stack.borrow_mut();
                span_stack.add_properties(local_span_handle, properties)
            })
        }
        self
    }
}

impl Drop for LocalSpanGuard {
    #[inline]
    fn drop(&mut self) {
        if let Some(span_handle) = self.span_handle.take() {
            LOCAL_SPAN_STACK.with(|span_stack| {
                let mut span_stack = span_stack.borrow_mut();
                span_stack.exit_span(span_handle);
            });
        }
    }
}
