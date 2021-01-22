// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::task::Poll;

use crate::{LocalSpan, Span};

impl<T: std::future::Future> FutureExt for T {}

pub trait FutureExt: Sized {
    /// Bind `span` to the future and return a future adaptor `WithSpan`. It can help trace a top
    /// future (aka task) by calling [`Span::try_enter`](Span::try_enter) when the executor
    /// [`poll`](std::future::Future::poll)s it.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main]
    /// # async fn main() {
    /// use minitrace::{Span, FutureExt};
    ///
    /// let (span, _collector) = Span::root("Task");
    /// let task = async {
    ///     42
    /// };
    ///
    /// tokio::spawn(task.in_span(span));
    /// # }
    /// ```
    #[inline]
    fn in_span(self, span: Span) -> InSpan<Self> {
        InSpan {
            inner: self,
            span: Some(span),
        }
    }

    /// Return a future adaptor `InNewSpan`. It will call [`Span::enter`](Span::enter) at the
    /// beginning of [`poll`](std::future::Future::poll)ing. A span will be generated at every
    /// single poll call. Note that polling on a future may return [`Poll::Pending`](Poll::Pending),
    /// so it can produce more than 1 span for the future.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main]
    /// # async fn main() {
    /// use minitrace::{Span, FutureExt};
    ///
    /// let (span, _collector) = Span::root("Task");
    ///
    /// let fut = async {
    ///     9527
    /// }.in_local_span("Future");
    ///
    /// let task = async {
    ///     fut.await
    /// };
    ///
    /// tokio::spawn(task.in_span(span));
    /// # }
    /// ```
    #[inline]
    fn in_local_span(self, event: &'static str) -> InLocalSpan<Self> {
        InLocalSpan { inner: self, event }
    }
}

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

        let _guard = this.span.as_ref().map(|s| s.try_enter());
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

#[pin_project::pin_project]
pub struct InLocalSpan<T> {
    #[pin]
    inner: T,
    event: &'static str,
}

impl<T: std::future::Future> std::future::Future for InLocalSpan<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = LocalSpan::enter(this.event);
        this.inner.poll(cx)
    }
}
