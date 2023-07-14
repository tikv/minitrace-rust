// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! Tools to trace a `Future`.
//!
//! [`FutureExt`] extends `Future` with two methods: [`in_span()`] and [`enter_on_poll()`].
//! The out-most future must use `in_span()`, otherwise, the tracing inside the future will be lost.
//!
//! # Example
//!
//! ```
//! use minitrace::prelude::*;
//!
//! let collector = {
//!     let (root, collector) = Span::root("root");
//!
//!     // To trace a task
//!     let task = async {
//!         async {
//!             // some work
//!         }
//!         .enter_on_poll("future is polled")
//!         .await;
//!     }
//!     .in_span(Span::enter_with_parent("task", &root));
//!
//!     # let runtime = tokio::runtime::Runtime::new().unwrap();
//!     runtime.spawn(task);
//! };
//! ```
//!
//! [`in_span()`]:(FutureExt::in_span)
//! [`enter_on_poll()`]:(FutureExt::enter_on_poll)

use std::task::Poll;

use crate::local::LocalSpan;
use crate::Span;

impl<T: std::future::Future> FutureExt for T {}

/// An extension trait for `Futures` that provides tracing instrument adapters.
pub trait FutureExt: std::future::Future + Sized {
    /// Bind a [`Span`] to the [`Future`] that keeps clocking until the future drops.
    ///
    /// Besides, it will set the span as the local parent at every poll so that `LocalSpan` becomes available inside the future.
    /// Under the hood, it call [`Span::set_local_parent`](Span::set_local_parent) when the executor [`poll`](std::future::Future::poll) it.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[tokio::main]
    /// # async fn main() {
    /// use minitrace::prelude::*;
    ///
    /// let (root, _collector) = Span::root("Root");
    /// let task = async { 42 }.in_span(Span::enter_with_parent("Task", &root));
    ///
    /// tokio::spawn(task);
    /// # }
    /// ```
    ///
    /// [`Future`]:(std::future::Future)
    #[inline]
    fn in_span(self, span: Span) -> InSpan<Self> {
        InSpan {
            inner: self,
            span: Some(span),
        }
    }

    /// Start a [`LocalSpan`] at every [`Future::poll()`]. It will create multiple _short_ spans if the future get polled multiple times.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[tokio::main]
    /// # async fn main() {
    /// use minitrace::prelude::*;
    ///
    /// let (root, _collector) = Span::root("Root");
    ///
    /// let fut = async { 9527 };
    ///
    /// let task = async { fut.enter_on_poll("Sub Task").await }
    ///     .in_span(Span::enter_with_parent("Task", &root));
    ///
    /// tokio::spawn(task);
    /// # }
    /// ```
    ///
    /// [`Future::poll()`]:(std::future::Future::poll)
    #[inline]
    fn enter_on_poll(self, name: &'static str) -> EnterOnPoll<Self> {
        EnterOnPoll { inner: self, name }
    }
}

/// Adapter for [`FutureExt::in_span()`](FutureExt::in_span).
#[pin_project::pin_project]
pub struct InSpan<T> {
    #[pin]
    inner: T,
    span: Option<Span>,
}

impl<T: std::future::Future> std::future::Future for InSpan<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let _guard = this.span.as_ref().map(|s| s.set_local_parent());
        let res = this.inner.poll(cx);

        match res {
            r @ Poll::Pending => r,
            other => {
                this.span.take();
                other
            }
        }
    }
}

/// Adapter for [`FutureExt::enter_on_poll()`](FutureExt::enter_on_poll).
#[pin_project::pin_project]
pub struct EnterOnPoll<T> {
    #[pin]
    inner: T,
    name: &'static str,
}

impl<T: std::future::Future> std::future::Future for EnterOnPoll<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = LocalSpan::enter_with_local_parent(this.name);
        this.inner.poll(cx)
    }
}
