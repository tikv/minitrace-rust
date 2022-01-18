// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::ParentSpan;
use crate::local::span_queue::{SpanHandle, SpanQueue};
use crate::util::{alloc_parent_spans, ParentSpans, RawSpans};

#[derive(Debug)]
pub(crate) struct SpanLine {
    span_queue: SpanQueue,
    epoch: usize,
    parents: Option<ParentSpans>,
}

impl SpanLine {
    pub fn new(capacity: usize, span_line_epoch: usize, parents: Option<ParentSpans>) -> Self {
        Self {
            span_queue: SpanQueue::with_capacity(capacity),
            epoch: span_line_epoch,
            parents,
        }
    }

    #[inline]
    pub fn span_line_epoch(&self) -> usize {
        self.epoch
    }

    #[inline]
    pub fn start_span(&mut self, event: &'static str) -> Option<LocalSpanHandle> {
        Some(LocalSpanHandle {
            span_handle: self.span_queue.start_span(event)?,
            span_line_epoch: self.epoch,
        })
    }

    #[inline]
    pub fn finish_span(&mut self, handle: LocalSpanHandle) {
        if self.epoch == handle.span_line_epoch {
            self.span_queue.finish_span(handle.span_handle);
        }
    }

    #[inline]
    pub fn add_properties<I, F>(&mut self, handle: &LocalSpanHandle, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        if self.epoch == handle.span_line_epoch {
            self.span_queue
                .add_properties(&handle.span_handle, properties());
        }
    }

    #[inline]
    pub fn current_parents(&self) -> Option<ParentSpans> {
        self.parents.as_ref().map(|parents| {
            let mut parents_spans = alloc_parent_spans();
            parents_spans.extend(parents.iter().map(|parent| ParentSpan {
                span_id: self.span_queue.current_span_id().unwrap_or(parent.span_id),
                collect_id: parent.collect_id,
            }));
            parents_spans
        })
    }

    #[inline]
    pub fn collect(self, span_line_epoch: usize) -> Option<(RawSpans, Option<ParentSpans>)> {
        (self.epoch == span_line_epoch).then(move || (self.span_queue.take_queue(), self.parents))
    }
}

#[derive(Debug)]
pub(crate) struct LocalSpanHandle {
    pub span_line_epoch: usize,
    span_handle: SpanHandle,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::span_id::SpanId;

    #[test]
    fn span_line_basic() {
        let mut span_line = SpanLine::new(16, 1, None);
        {
            let span1 = span_line.start_span("span1").unwrap();
            {
                let span2 = span_line.start_span("span2").unwrap();
                {
                    let span3 = span_line.start_span("span3").unwrap();
                    span_line.add_properties(&span3, || [("k1", "v1".to_owned())]);
                    span_line.finish_span(span3);
                }
                span_line.finish_span(span2);
            }
            span_line.finish_span(span1);
        }
        let (spans, collect_parents) = span_line.collect(1).unwrap();
        assert!(collect_parents.is_none());

        let mut raw_spans = spans.into_inner().1;
        raw_spans.sort_unstable_by(|a, b| a.id.0.cmp(&b.id.0));
        assert_eq!(raw_spans.len(), 3);
        assert_eq!(raw_spans[0].event, "span1");
        assert_eq!(raw_spans[1].event, "span2");
        assert_eq!(raw_spans[2].event, "span3");
        assert_eq!(raw_spans[2].properties, vec![("k1", "v1".to_owned())]);
    }

    #[test]
    fn current_parents() {
        let mut parents = alloc_parent_spans();
        let parent1 = ParentSpan {
            span_id: SpanId::new(9527),
            collect_id: 42,
        };
        let parent2 = ParentSpan {
            span_id: SpanId::new(9528),
            collect_id: 43,
        };
        parents.extend([parent1, parent2]);
        let mut span_line = SpanLine::new(16, 1, Some(parents));

        let current_parents = span_line.current_parents().unwrap();
        assert_eq!(current_parents.len(), 2);
        assert_eq!(current_parents[0], parent1);
        assert_eq!(current_parents[1], parent2);

        let span = span_line.start_span("span").unwrap();
        let current_parents = span_line.current_parents().unwrap();
        assert_eq!(current_parents.len(), 2);
        assert_eq!(
            current_parents[0],
            ParentSpan {
                span_id: span_line.span_queue.current_span_id().unwrap(),
                collect_id: 42
            }
        );
        assert_eq!(
            current_parents[1],
            ParentSpan {
                span_id: span_line.span_queue.current_span_id().unwrap(),
                collect_id: 43
            }
        );
        span_line.finish_span(span);

        let current_parents = span_line.current_parents().unwrap();
        assert_eq!(current_parents.len(), 2);
        assert_eq!(current_parents[0], parent1);
        assert_eq!(current_parents[1], parent2);

        let (spans, collect_parents) = span_line.collect(1).unwrap();
        let collect_parents = collect_parents.unwrap();
        assert_eq!(collect_parents.len(), 2);
        assert_eq!(collect_parents[0], parent1);
        assert_eq!(collect_parents[1], parent2);
        assert_eq!(spans.into_inner().1.len(), 1);
    }

    #[test]
    fn unmatched_epoch_add_properties() {
        let mut span_line1 = SpanLine::new(16, 1, None);
        let mut span_line2 = SpanLine::new(16, 2, None);
        assert_eq!(span_line1.span_line_epoch(), 1);
        assert_eq!(span_line2.span_line_epoch(), 2);

        let span = span_line1.start_span("span").unwrap();
        span_line2.add_properties(&span, || [("k1", "v1".to_owned())]);
        span_line1.finish_span(span);

        let raw_spans = span_line1.collect(1).unwrap().0.into_inner().1;
        assert_eq!(raw_spans.len(), 1);
        assert_eq!(raw_spans[0].properties.len(), 0);

        let raw_spans = span_line2.collect(2).unwrap().0.into_inner().1;
        assert!(raw_spans.is_empty());
    }

    #[test]
    fn unmatched_epoch_finish_span() {
        let mut parents1 = alloc_parent_spans();
        let parent = ParentSpan {
            span_id: SpanId::default(),
            collect_id: 42,
        };
        parents1.push(parent);
        let mut span_line1 = SpanLine::new(16, 1, Some(parents1));
        let mut span_line2 = SpanLine::new(16, 2, None);
        assert_eq!(span_line1.span_line_epoch(), 1);
        assert_eq!(span_line2.span_line_epoch(), 2);

        let span = span_line1.start_span("span").unwrap();
        let parents_before_finish = span_line1.current_parents().unwrap();
        assert_eq!(parents_before_finish.len(), 1);
        span_line2.finish_span(span);

        let parents_after_finish = span_line1.current_parents().unwrap();
        assert_eq!(parents_after_finish.len(), 1);
        assert_eq!(parents_before_finish[0], parents_after_finish[0]);

        let (spans, collect_parents) = span_line1.collect(1).unwrap();
        let collect_parents = collect_parents.unwrap();
        assert_eq!(collect_parents.len(), 1);
        assert_eq!(collect_parents[0], parent);
        assert_eq!(spans.into_inner().1.len(), 1);
        let (spans, collect_parents) = span_line2.collect(2).unwrap();
        assert!(collect_parents.is_none());
        assert!(spans.into_inner().1.is_empty());
    }

    #[test]
    fn unmatched_epoch_collect() {
        let span_line1 = SpanLine::new(16, 1, None);
        let span_line2 = SpanLine::new(16, 2, None);
        assert_eq!(span_line1.span_line_epoch(), 1);
        assert_eq!(span_line2.span_line_epoch(), 2);
        assert!(span_line1.collect(2).is_none());
        assert!(span_line2.collect(1).is_none());
    }
}
