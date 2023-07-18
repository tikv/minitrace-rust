// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::rc::Rc;

use crate::local::local_span_stack::LOCAL_SPAN_STACK;
use crate::Span;

/// An event that represents a single point in time during the execution of a span.
pub struct Event;

impl Event {
    /// Adds an event to the parent span with the given name and properties.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
    ///
    /// Event::add_to_parent("event in root", &root, || [("key", "value".to_owned())]);
    /// ```
    pub fn add_to_parent<I, F>(name: &'static str, parent: &Span, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        #[cfg(feature = "report")]
        {
            let mut span = Span::enter_with_parent(name, parent).with_properties(properties);
            if let Some(mut inner) = span.inner.take() {
                inner.raw_span.is_event = true;
                inner.submit_spans();
            }
        }
    }

    /// Adds an event to the current local parent span with the given name and properties.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
    /// let _guard = root.set_local_parent();
    ///
    /// Event::add_to_local_parent("event in root", || [("key", "value".to_owned())]);
    /// ```
    pub fn add_to_local_parent<I, F>(name: &'static str, properties: F)
    where
        I: IntoIterator<Item = (&'static str, String)>,
        F: FnOnce() -> I,
    {
        #[cfg(feature = "report")]
        {
            let stack = LOCAL_SPAN_STACK.with(Rc::clone);
            let mut stack = stack.borrow_mut();
            stack.add_event(name, properties);
        }
    }
}
