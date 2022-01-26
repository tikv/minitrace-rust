// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::local_span_line::LocalSpanHandle;
use crate::local::local_span_stack::{LocalSpanStack, LOCAL_SPAN_STACK};

use std::cell::RefCell;
use std::rc::Rc;

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
    pub fn add_property<F>(&mut self, property: F)
    where
        F: FnOnce() -> (&'static str, String),
    {
        self.add_properties(|| [property()]);
    }

    #[inline]
    pub fn add_properties<I, F>(&mut self, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        if let Some(LocalSpanInner { stack, span_handle }) = &self.inner {
            let span_stack = &mut *stack.borrow_mut();
            span_stack.add_properties(span_handle, properties);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::CollectTokenItem;
    use crate::local::local_span_stack::LocalSpanStack;
    use crate::local::span_id::SpanId;
    use crate::local::LocalCollector;
    use crate::util::tree::tree_str_from_raw_spans;

    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn local_span_basic() {
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));

        let token = CollectTokenItem {
            parent_id_of_roots: SpanId::new(9527),
            collect_id: 42,
        };
        let collector = LocalCollector::new(Some(token.into()), stack.clone());

        {
            let _g = LocalSpan::enter_with_stack("span1", stack.clone());
            {
                let mut span = LocalSpan::enter_with_stack("span2", stack);
                span.add_property(|| ("k1", "v1".to_owned()));
            }
        }

        let (spans, collect_token) = collector.collect_spans_and_token();
        assert_eq!(collect_token.unwrap().as_slice(), &[token]);
        assert_eq!(
            tree_str_from_raw_spans(spans.spans),
            r#"
span1 []
    span2 [("k1", "v1")]
"#
        );
    }

    #[test]
    #[should_panic]
    fn drop_out_of_order() {
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));

        let token = CollectTokenItem {
            parent_id_of_roots: SpanId::new(9527),
            collect_id: 42,
        };
        let collector = LocalCollector::new(Some(token.into()), stack.clone());

        {
            let span1 = LocalSpan::enter_with_stack("span1", stack.clone());
            {
                let mut span2 = LocalSpan::enter_with_stack("span2", stack);
                span2.add_property(|| ("k1", "v1".to_owned()));

                drop(span1);
            }
        }

        let _ = collector.collect_spans_and_token();
    }
}
