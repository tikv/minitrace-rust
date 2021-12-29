// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::task::Poll;

use crate::local::LocalSpan;
use crate::span::Span;

impl<T: std::future::Future> FutureExt for T {}

pub trait FutureExt: Sized {
    /// Bind a `span` to the future to record the entire lifetime of future. This is usually used
    /// on the outmost async block.
    ///
    /// It'll call [`Span::set_local_parent`](Span::set_local_parent) when the executor
    /// [`poll`](std::future::Future::poll) it.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main]
    /// # async fn main() {
    /// use minitrace::prelude::*;
    ///
    /// let (root, _collector) = Span::root("Root");
    /// let task = async {
    ///     42
    /// }
    /// .in_span(Span::enter_with_parent("Task", &root));
    ///
    /// tokio::spawn(task);
    /// # }
    /// ```
    #[inline]
    fn in_span(self, parent: Span) -> InSpan<Self> {
        InSpan {
            inner: self,
            span: Some(parent),
        }
    }

    /// It will call [`LocalSpan::enter_with_local_parent`](LocalSpan::enter_with_local_parent) at the
    /// beginning of [`poll`](std::future::Future::poll)ing. A span will be generated at every
    /// single poll call. Note that polling on a future may return [`Poll::Pending`](Poll::Pending),
    /// so it can produce more than 1 span for the future.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main]
    /// # async fn main() {
    /// use minitrace::prelude::*;
    ///
    /// let (root, _collector) = Span::root("Root");
    ///
    /// let fut = async {
    ///     9527
    /// };
    ///
    /// let task = async {
    ///     fut.enter_on_poll("Sub Task").await
    /// }
    /// .in_span(Span::enter_with_parent("Task", &root));
    ///
    /// tokio::spawn(task);
    /// # }
    /// ```
    ///
    #[inline]
    fn enter_on_poll(self, event: &'static str) -> EnterOnPoll<Self> {
        EnterOnPoll { inner: self, event }
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

#[pin_project::pin_project]
pub struct EnterOnPoll<T> {
    #[pin]
    inner: T,
    event: &'static str,
}

impl<T: std::future::Future> std::future::Future for EnterOnPoll<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = LocalSpan::enter_with_local_parent(this.event);
        this.inner.poll(cx)
    }
}
