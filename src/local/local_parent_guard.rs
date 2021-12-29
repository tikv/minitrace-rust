// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::collector::acquirer::{Acquirer, SpanCollection};
use crate::local::local_collector::LocalCollector;
use crate::local::local_span_line::LOCAL_SPAN_LINE;
use crate::local::span_id::SpanId;
use crate::span::Span;

thread_local! {
    static ATTACHED_SPAN: RefCell<Option<AttachedSpan>> = RefCell::new(None);
}

pub struct AttachedSpan {
    span_id: SpanId,
    acquirers: Vec<Acquirer>,

    local_collector: LocalCollector,
}

impl AttachedSpan {
    pub fn new_child_span(event: &'static str) -> Option<Span> {
        let local_parent_span_id =
            LOCAL_SPAN_LINE.with(|span_line| span_line.borrow().span_queue.next_parent_id);

        ATTACHED_SPAN.with(|attached_span| {
            let attached_span = attached_span.borrow();
            if let Some(AttachedSpan {
                span_id: attached_span_id,
                acquirers,
                ..
            }) = attached_span.as_ref()
            {
                Some(Span::new(
                    acquirers
                        .iter()
                        .map(|acq| (local_parent_span_id.unwrap_or(*attached_span_id), acq)),
                    event,
                ))
            } else {
                None
            }
        })
    }

    #[inline]
    pub fn is_occupied() -> bool {
        ATTACHED_SPAN.with(|attached_span| {
            let attached_span = attached_span.borrow();
            attached_span.is_some()
        })
    }
}

#[must_use]
pub struct LocalParentGuard {
    // Identical to
    // ```
    // impl !Sync for SpanGuard {}
    // impl !Send for SpanGuard {}
    // ```
    //
    // TODO: Replace it once feature `negative_impls` is stable.
    _p: PhantomData<*const ()>,
}

impl Drop for LocalParentGuard {
    fn drop(&mut self) {
        ATTACHED_SPAN.with(|attached_span| {
            if let Some(AttachedSpan {
                span_id,
                acquirers,
                local_collector,
            }) = attached_span.borrow_mut().take()
            {
                let raw_spans = Arc::new(local_collector.collect());
                for acq in acquirers {
                    acq.submit(SpanCollection::LocalSpans {
                        local_spans: raw_spans.clone(),
                        parent_id_of_root: span_id,
                    })
                }
            }
        })
    }
}

impl LocalParentGuard {
    #[inline]
    pub(crate) fn new_with_local_collector(span: &Span, local_collector: LocalCollector) -> Self {
        ATTACHED_SPAN.with(|attached_span| {
            let mut attached_span = attached_span.borrow_mut();

            if attached_span.is_some() {
                panic!("Attach too much spans: > 1")
            }

            if let Some(inner) = &span.inner {
                *attached_span = Some(AttachedSpan {
                    span_id: inner.span_id,
                    acquirers: inner.to_report.iter().map(|(_, acq)| acq.clone()).collect(),
                    local_collector,
                })
            }
        });

        LocalParentGuard {
            _p: Default::default(),
        }
    }
}
