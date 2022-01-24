// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::local_span_line::{LocalSpanHandle, SpanLine};
use crate::util::{CollectToken, RawSpans};

use std::cell::RefCell;
use std::rc::Rc;

const DEFAULT_SPAN_STACK_SIZE: usize = 4096;
const DEFAULT_SPAN_QUEUE_SIZE: usize = 10240;

thread_local! {
    pub static LOCAL_SPAN_STACK: Rc<RefCell<LocalSpanStack>> = Rc::new(RefCell::new(LocalSpanStack::with_capacity(DEFAULT_SPAN_STACK_SIZE)));
}

#[derive(Debug)]
pub struct LocalSpanStack {
    span_lines: Vec<SpanLine>,
    capacity: usize,
    next_span_line_epoch: usize,
}

impl LocalSpanStack {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            span_lines: Vec::with_capacity(capacity / 8),
            capacity,
            next_span_line_epoch: 0,
        }
    }

    #[inline]
    pub fn enter_span(&mut self, event: &'static str) -> Option<LocalSpanHandle> {
        let span_line = self.current_span_line()?;
        span_line.start_span(event)
    }

    #[inline]
    pub fn exit_span(&mut self, local_span_handle: LocalSpanHandle) {
        if let Some(span_line) = self.current_span_line() {
            debug_assert_eq!(
                span_line.span_line_epoch(),
                local_span_handle.span_line_epoch
            );
            span_line.finish_span(local_span_handle);
        }
    }

    /// Register a new span line to the span stack. If succeed, return a span line epoch which can
    /// be used to unregister the span line via [`LocalSpanStack::unregister_and_collect`]. If
    /// the size of the span stack is greater than the `capacity`, registration will fail
    /// and a `None` will be returned.
    ///
    /// [`LocalSpanStack::unregister_and_collect`](LocalSpanStack::unregister_and_collect)
    #[inline]
    pub fn register_span_line(
        &mut self,
        collect_token: Option<CollectToken>,
    ) -> Option<SpanLineHandle> {
        if self.span_lines.len() >= self.capacity {
            return None;
        }

        let epoch = self.next_span_line_epoch;
        self.next_span_line_epoch = self.next_span_line_epoch.wrapping_add(1);

        let span_line = SpanLine::new(DEFAULT_SPAN_QUEUE_SIZE, epoch, collect_token);
        self.span_lines.push(span_line);
        Some(SpanLineHandle {
            span_line_epoch: epoch,
        })
    }

    pub fn unregister_and_collect(
        &mut self,
        span_line_handle: SpanLineHandle,
    ) -> Option<(RawSpans, Option<CollectToken>)> {
        debug_assert_eq!(
            self.current_span_line().unwrap().span_line_epoch(),
            span_line_handle.span_line_epoch,
        );
        let span_line = self.span_lines.pop()?;
        span_line.collect(span_line_handle.span_line_epoch)
    }

    pub fn current_collect_token(&mut self) -> Option<CollectToken> {
        let span_line = self.current_span_line()?;
        span_line.current_collect_token()
    }

    #[inline]
    pub fn add_properties<I, F>(&mut self, local_span_handle: &LocalSpanHandle, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        debug_assert!(self.current_span_line().is_some());
        if let Some(span_line) = self.current_span_line() {
            debug_assert_eq!(
                span_line.span_line_epoch(),
                local_span_handle.span_line_epoch
            );
            span_line.add_properties(local_span_handle, properties);
        }
    }

    #[inline]
    fn current_span_line(&mut self) -> Option<&mut SpanLine> {
        self.span_lines.last_mut()
    }
}

#[derive(Debug)]
pub struct SpanLineHandle {
    span_line_epoch: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::CollectTokenItem;
    use crate::local::span_id::SpanId;
    use crate::util::tree::{t, Tree};

    #[test]
    fn span_stack_basic() {
        let mut span_stack = LocalSpanStack::with_capacity(16);

        let token1 = CollectTokenItem {
            parent_id_of_roots: SpanId::new(9527),
            collect_id: 42,
        };
        let span_line1 = span_stack.register_span_line(token1.into()).unwrap();
        {
            {
                let span1 = span_stack.enter_span("span1").unwrap();
                {
                    let span2 = span_stack.enter_span("span2").unwrap();
                    span_stack.exit_span(span2);
                }
                span_stack.exit_span(span1);
            }

            let token2 = CollectTokenItem {
                parent_id_of_roots: SpanId::new(9528),
                collect_id: 48,
            };
            let span_line2 = span_stack.register_span_line(token2.into()).unwrap();
            {
                let span3 = span_stack.enter_span("span3").unwrap();
                {
                    let span4 = span_stack.enter_span("span4").unwrap();
                    span_stack.exit_span(span4);
                }
                span_stack.exit_span(span3);
            }

            let (spans, collect_token) = span_stack.unregister_and_collect(span_line2).unwrap();
            assert_eq!(collect_token.unwrap().as_slice(), &[token2]);
            assert_eq!(
                Tree::from_raw_spans(spans).as_slice(),
                &[t("span3", [t("span4", [])])]
            );
        }

        let (spans, collect_token) = span_stack.unregister_and_collect(span_line1).unwrap();
        assert_eq!(collect_token.unwrap().as_slice(), &[token1]);
        assert_eq!(
            Tree::from_raw_spans(spans).as_slice(),
            &[t("span1", [t("span2", [])])]
        );
    }

    #[test]
    fn span_stack_is_full() {
        let mut span_stack = LocalSpanStack::with_capacity(4);

        let span_line1 = span_stack.register_span_line(None).unwrap();
        {
            let span_line2 = span_stack.register_span_line(None).unwrap();
            {
                let span_line3 = span_stack
                    .register_span_line(
                        CollectTokenItem {
                            parent_id_of_roots: SpanId::new(9527),
                            collect_id: 42,
                        }
                        .into(),
                    )
                    .unwrap();
                {
                    let span_line4 = span_stack.register_span_line(None).unwrap();
                    {
                        assert!(span_stack
                            .register_span_line(
                                CollectTokenItem {
                                    parent_id_of_roots: SpanId::new(9528),
                                    collect_id: 43
                                }
                                .into()
                            )
                            .is_none());
                        assert!(span_stack.register_span_line(None).is_none());
                    }
                    let _ = span_stack.unregister_and_collect(span_line4).unwrap();
                }
                {
                    let span_line5 = span_stack.register_span_line(None).unwrap();
                    {
                        assert!(span_stack
                            .register_span_line(
                                CollectTokenItem {
                                    parent_id_of_roots: SpanId::new(9529),
                                    collect_id: 44
                                }
                                .into()
                            )
                            .is_none());
                        assert!(span_stack.register_span_line(None).is_none());
                    }
                    let _ = span_stack.unregister_and_collect(span_line5).unwrap();
                }
                let _ = span_stack.unregister_and_collect(span_line3).unwrap();
            }
            let _ = span_stack.unregister_and_collect(span_line2).unwrap();
        }
        let _ = span_stack.unregister_and_collect(span_line1).unwrap();
    }

    #[test]
    fn current_collect_token() {
        let mut span_stack = LocalSpanStack::with_capacity(16);
        assert!(span_stack.current_collect_token().is_none());
        let token1 = CollectTokenItem {
            parent_id_of_roots: SpanId::new(1),
            collect_id: 1,
        };
        let span_line1 = span_stack.register_span_line(token1.into()).unwrap();
        assert_eq!(
            span_stack.current_collect_token().unwrap().as_slice(),
            &[token1]
        );
        {
            let span_line2 = span_stack.register_span_line(None).unwrap();
            assert!(span_stack.current_collect_token().is_none());
            {
                let token3 = CollectTokenItem {
                    parent_id_of_roots: SpanId::new(3),
                    collect_id: 3,
                };
                let span_line3 = span_stack.register_span_line(token3.into()).unwrap();
                assert_eq!(
                    span_stack.current_collect_token().unwrap().as_slice(),
                    &[token3]
                );
                let _ = span_stack.unregister_and_collect(span_line3).unwrap();
            }
            assert!(span_stack.current_collect_token().is_none());
            let _ = span_stack.unregister_and_collect(span_line2).unwrap();

            let token4 = CollectTokenItem {
                parent_id_of_roots: SpanId::new(4),
                collect_id: 4,
            };
            let span_line4 = span_stack.register_span_line(token4.into()).unwrap();
            assert_eq!(
                span_stack.current_collect_token().unwrap().as_slice(),
                &[token4]
            );
            let _ = span_stack.unregister_and_collect(span_line4).unwrap();
        }
        assert_eq!(
            span_stack.current_collect_token().unwrap().as_slice(),
            &[token1]
        );
        let _ = span_stack.unregister_and_collect(span_line1).unwrap();
        assert!(span_stack.current_collect_token().is_none());
    }

    #[test]
    #[should_panic]
    fn unmatched_span_line_exit_span() {
        let mut span_stack = LocalSpanStack::with_capacity(16);
        let span_line1 = span_stack.register_span_line(None).unwrap();
        let span1 = span_stack.enter_span("span1").unwrap();
        {
            let span_line2 = span_stack
                .register_span_line(
                    CollectTokenItem {
                        parent_id_of_roots: SpanId::new(9527),
                        collect_id: 42,
                    }
                    .into(),
                )
                .unwrap();
            span_stack.exit_span(span1);
            let _ = span_stack.unregister_and_collect(span_line2).unwrap();
        }
        let _ = span_stack.unregister_and_collect(span_line1).unwrap();
    }

    #[test]
    #[should_panic]
    fn unmatched_span_line_add_properties() {
        let mut span_stack = LocalSpanStack::with_capacity(16);
        let span_line1 = span_stack.register_span_line(None).unwrap();
        let span1 = span_stack.enter_span("span1").unwrap();
        {
            let span_line2 = span_stack
                .register_span_line(
                    CollectTokenItem {
                        parent_id_of_roots: SpanId::new(9527),
                        collect_id: 42,
                    }
                    .into(),
                )
                .unwrap();
            span_stack.add_properties(&span1, || [("k1", "v1".to_owned())]);
            let _ = span_stack.unregister_and_collect(span_line2).unwrap();
        }
        span_stack.exit_span(span1);
        let _ = span_stack.unregister_and_collect(span_line1).unwrap();
    }

    #[test]
    #[should_panic]
    fn unmatched_span_line_collect() {
        let mut span_stack = LocalSpanStack::with_capacity(16);
        let span_line1 = span_stack.register_span_line(None).unwrap();
        {
            let span_line2 = span_stack
                .register_span_line(
                    CollectTokenItem {
                        parent_id_of_roots: SpanId::new(9527),
                        collect_id: 42,
                    }
                    .into(),
                )
                .unwrap();
            let _ = span_stack.unregister_and_collect(span_line1).unwrap();
            let _ = span_stack.unregister_and_collect(span_line2).unwrap();
        }
    }
}
