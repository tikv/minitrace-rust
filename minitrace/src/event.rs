// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::rc::Rc;

use crate::local::local_span_stack::LOCAL_SPAN_STACK;
use crate::Span;

/// `Event`s represent single points in time where something occurred during the execution of a program.
///
/// An `Event` can be compared to a log record in unstructured logging, but with two key differences:
///
/// - `Event`s exist within the context of a span. Unlike log lines, they may be located within the trace tree,
///   allowing visibility into the temporal context in which the event occurred, as well as the source code location.
/// - Like spans, Events have structured key-value data known as properties, which may include textual message.
pub struct Event;

impl Event {
    /// Add an event to the parent span.
    pub fn add_to_parent<I, F>(name: &'static str, parent: &Span, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        let mut span = Span::enter_with_parent(name, parent);
        span.add_properties(properties);
        if let Some(mut inner) = span.inner.take() {
            inner.raw_span.is_event = true;
            inner.submit_spans();
        }
    }

    /// Add an event to the current local parent span.
    pub fn add_to_local_parent<I, F>(name: &'static str, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        let stack = LOCAL_SPAN_STACK.with(Rc::clone);
        let mut stack = stack.borrow_mut();
        stack.add_event(name, properties);
    }
}
