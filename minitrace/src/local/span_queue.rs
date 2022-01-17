// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::raw_span::RawSpan;
use crate::local::span_id::{DefaultIdGenerator, SpanId};
use crate::util::{alloc_raw_spans, RawSpans};

use minstant::Instant;

const DEFAULT_SPAN_QUEUE_SIZE: usize = 10240;

#[derive(Debug)]
pub(crate) struct SpanQueue {
    span_queue: RawSpans,
    capacity: usize,
    pub(crate) next_parent_id: Option<SpanId>,
}

pub(crate) struct SpanHandle {
    pub(crate) index: usize,
}

impl SpanQueue {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_SPAN_QUEUE_SIZE)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        let span_queue = alloc_raw_spans();
        Self {
            span_queue,
            capacity,
            next_parent_id: None,
        }
    }

    #[inline]
    pub fn start_span(&mut self, event: &'static str) -> Option<SpanHandle> {
        if self.span_queue.len() >= self.capacity {
            return None;
        }

        let span = RawSpan::begin_with(
            DefaultIdGenerator::next_id(),
            self.next_parent_id.unwrap_or(SpanId(0)),
            Instant::now(),
            event,
        );
        self.next_parent_id = Some(span.id);

        let index = self.span_queue.len();
        self.span_queue.push(span);

        Some(SpanHandle { index })
    }

    #[inline]
    pub fn finish_span(&mut self, span_handle: SpanHandle) {
        debug_assert!(span_handle.index < self.span_queue.len());
        debug_assert_eq!(
            self.next_parent_id,
            Some(self.span_queue[span_handle.index].id)
        );

        let span = &mut self.span_queue[span_handle.index];
        span.end_with(Instant::now());

        self.next_parent_id = Some(span.parent_id).filter(|id| id.0 != 0);
    }

    #[inline]
    pub fn add_properties<I: IntoIterator<Item = (&'static str, String)>>(
        &mut self,
        span_handle: &SpanHandle,
        properties: I,
    ) {
        debug_assert!(span_handle.index < self.span_queue.len());

        let span = &mut self.span_queue[span_handle.index];
        span.properties.extend(properties);
    }

    #[inline]
    pub fn take_queue(self) -> RawSpans {
        self.span_queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_queue_basic() {
        let mut queue = SpanQueue::with_capacity(16);
        {
            let span1 = queue.start_span("span1").unwrap();
            {
                let span2 = queue.start_span("span2").unwrap();
                {
                    let span3 = queue.start_span("span3").unwrap();
                    queue.finish_span(span3);
                }
                queue.finish_span(span2);
            }
            queue.finish_span(span1);
        }
        let mut raw_spans = queue.take_queue().into_inner().1;
        raw_spans.sort_unstable_by(|a, b| a.id.0.cmp(&b.id.0));
        assert_eq!(raw_spans.len(), 3);
        assert_eq!(raw_spans[0].event, "span1");
        assert_eq!(raw_spans[0].parent_id, SpanId(0));
        assert_eq!(raw_spans[1].event, "span2");
        assert_eq!(raw_spans[1].parent_id, raw_spans[0].id);
        assert_eq!(raw_spans[2].event, "span3");
        assert_eq!(raw_spans[2].parent_id, raw_spans[1].id);
    }

    #[test]
    fn span_add_properties() {
        let mut queue = SpanQueue::with_capacity(16);
        {
            let span1 = queue.start_span("span1").unwrap();
            queue.add_properties(
                &span1,
                vec![("k1", "v1".to_owned()), ("k2", "v2".to_owned())].into_iter(),
            );
            {
                let span2 = queue.start_span("span2").unwrap();
                queue.add_properties(&span2, vec![("k1", "v1".to_owned())].into_iter());
                queue.finish_span(span2);
            }
            queue.finish_span(span1);
        }
        let mut raw_spans = queue.take_queue().into_inner().1;
        raw_spans.sort_unstable_by(|a, b| a.id.0.cmp(&b.id.0));
        assert_eq!(raw_spans.len(), 2);
        assert_eq!(raw_spans[0].event, "span1");
        assert_eq!(
            raw_spans[0].properties,
            vec![("k1", "v1".to_owned()), ("k2", "v2".to_owned())]
        );
        assert_eq!(raw_spans[1].event, "span2");
        assert_eq!(raw_spans[1].properties, vec![("k1", "v1".to_owned())]);
    }

    #[test]
    fn span_not_finished() {
        let mut queue = SpanQueue::with_capacity(16);
        {
            let _span1 = queue.start_span("span1").unwrap();
            {
                let _span2 = queue.start_span("span2").unwrap();
                {
                    let _span3 = queue.start_span("span3").unwrap();
                }
            }
        }
        let mut raw_spans = queue.take_queue().into_inner().1;
        raw_spans.sort_unstable_by(|a, b| a.id.0.cmp(&b.id.0));
        assert_eq!(raw_spans.len(), 3);
        assert_eq!(raw_spans[0].event, "span1");
        assert_eq!(raw_spans[0].parent_id, SpanId(0));
        assert_eq!(raw_spans[1].event, "span2");
        assert_eq!(raw_spans[1].parent_id, raw_spans[0].id);
        assert_eq!(raw_spans[2].event, "span3");
        assert_eq!(raw_spans[2].parent_id, raw_spans[1].id);
    }

    #[test]
    #[should_panic]
    fn finish_span_out_of_order() {
        let mut queue = SpanQueue::with_capacity(16);
        let span1 = queue.start_span("span1").unwrap();
        let span2 = queue.start_span("span2").unwrap();
        queue.finish_span(span1);
        queue.finish_span(span2);
    }

    #[test]
    fn span_queue_out_of_size() {
        let mut queue = SpanQueue::with_capacity(4);
        {
            let span1 = queue.start_span("span1").unwrap();
            {
                let span2 = queue.start_span("span2").unwrap();
                {
                    let span3 = queue.start_span("span3").unwrap();
                    {
                        let span4 = queue.start_span("span4").unwrap();
                        assert!(queue.start_span("span5").is_none());
                        queue.finish_span(span4);
                    }
                    assert!(queue.start_span("span5").is_none());
                    queue.finish_span(span3);
                }
                assert!(queue.start_span("span5").is_none());
                queue.finish_span(span2);
            }
            assert!(queue.start_span("span5").is_none());
            queue.finish_span(span1);
        }
        assert!(queue.start_span("span5").is_none());

        let mut raw_spans = queue.take_queue().into_inner().1;
        raw_spans.sort_unstable_by(|a, b| a.id.0.cmp(&b.id.0));
        assert_eq!(raw_spans.len(), 4);
        assert_eq!(raw_spans[0].event, "span1");
        assert_eq!(raw_spans[0].parent_id, SpanId(0));
        assert_eq!(raw_spans[1].event, "span2");
        assert_eq!(raw_spans[1].parent_id, raw_spans[0].id);
        assert_eq!(raw_spans[2].event, "span3");
        assert_eq!(raw_spans[2].parent_id, raw_spans[1].id);
        assert_eq!(raw_spans[3].event, "span4");
        assert_eq!(raw_spans[3].parent_id, raw_spans[2].id);
    }

    #[test]
    fn complicated_relationship() {
        let mut queue = SpanQueue::with_capacity(16);
        {
            let span1 = queue.start_span("span1").unwrap();
            queue.finish_span(span1);
        }
        {
            let span2 = queue.start_span("span2").unwrap();
            {
                let span3 = queue.start_span("span3").unwrap();
                queue.finish_span(span3);
            }
            {
                let span4 = queue.start_span("span4").unwrap();
                {
                    let span5 = queue.start_span("span5").unwrap();
                    {
                        let span6 = queue.start_span("span6").unwrap();
                        queue.finish_span(span6);
                    }
                    queue.finish_span(span5);
                }
                queue.finish_span(span4);
            }
            queue.finish_span(span2);
        }
        {
            let span7 = queue.start_span("span7").unwrap();
            queue.finish_span(span7);
        }
        let mut raw_spans = queue.take_queue().into_inner().1;
        raw_spans.sort_unstable_by(|a, b| a.id.0.cmp(&b.id.0));
        assert_eq!(raw_spans.len(), 7);
        assert_eq!(raw_spans[0].event, "span1");
        assert_eq!(raw_spans[0].parent_id, SpanId(0));
        assert_eq!(raw_spans[1].event, "span2");
        assert_eq!(raw_spans[1].parent_id, SpanId(0));
        assert_eq!(raw_spans[2].event, "span3");
        assert_eq!(raw_spans[2].parent_id, raw_spans[1].id);
        assert_eq!(raw_spans[3].event, "span4");
        assert_eq!(raw_spans[3].parent_id, raw_spans[1].id);
        assert_eq!(raw_spans[4].event, "span5");
        assert_eq!(raw_spans[4].parent_id, raw_spans[3].id);
        assert_eq!(raw_spans[5].event, "span6");
        assert_eq!(raw_spans[5].parent_id, raw_spans[4].id);
        assert_eq!(raw_spans[6].event, "span7");
        assert_eq!(raw_spans[6].parent_id, SpanId(0));
    }
}
