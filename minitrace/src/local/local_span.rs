// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;
use std::rc::Rc;

use crate::local::local_span_line::{LocalSpanHandle, LocalSpanStack, LOCAL_SPAN_STACK};

#[must_use]
pub struct LocalSpan {
    inner: Option<LocalSpanInner>,
}

struct LocalSpanInner {
    stack: Rc<RefCell<LocalSpanStack>>,
    span_handle: LocalSpanHandle,
}

impl LocalSpan {
    #[inline]
    pub fn enter_with_local_parent(event: &'static str) -> Self {
        let stack = LOCAL_SPAN_STACK.with(Rc::clone);
        Self::enter_with_stack(event, stack)
    }

    #[inline]
    #[must_use]
    #[allow(clippy::double_must_use)]
    pub fn with_property<F>(&mut self, property: F) -> &mut Self
    where
        F: FnOnce() -> (&'static str, String),
    {
        self.with_properties(|| [property()])
    }

    #[inline]
    #[must_use]
    #[allow(clippy::double_must_use)]
    pub fn with_properties<I, F>(&mut self, properties: F) -> &mut Self
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        if let Some(LocalSpanInner { stack, span_handle }) = &self.inner {
            let span_stack = &mut *stack.borrow_mut();
            span_stack.add_properties(span_handle, properties)
        }
        self
    }
}

impl LocalSpan {
    #[inline]
    pub(crate) fn enter_with_stack(
        event: &'static str,
        stack: Rc<RefCell<LocalSpanStack>>,
    ) -> Self {
        let span_handle = {
            let mut stack = stack.borrow_mut();
            stack.enter_span(event)
        };

        let inner = span_handle.map(|span_handle| LocalSpanInner { stack, span_handle });

        Self { inner }
    }
}

impl Drop for LocalSpan {
    #[inline]
    fn drop(&mut self) {
        if let Some(LocalSpanInner { stack, span_handle }) = self.inner.take() {
            let mut span_stack = stack.borrow_mut();
            span_stack.exit_span(span_handle);
        }
    }
}
