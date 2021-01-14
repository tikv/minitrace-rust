// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::task::Poll;

use crate::{Scope, Span};

impl<T: Sized> FutureExt for T {}

pub trait FutureExt: Sized {
    /// Bind `scope` to the future and return a future adaptor `WithScope`. It can help trace a top
    /// future (aka task) by calling [`Scope::try_enter`](Scope::try_enter) when the executor
    /// [`poll`](std::future::Future::poll)s it.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[tokio::main]
    /// # async fn main() {
    /// use minitrace::{Scope, FutureExt};
    ///
    /// let (scope, _collector) = Scope::root("Task");
    /// let task = async {
    ///     42
    /// };
    ///
    /// tokio::spawn(task.with_scope(scope));
    /// # }
    /// ```
    #[inline]
    fn with_scope(self, scope: Scope) -> WithScope<Self> {
        WithScope {
            inner: self,
            scope: Some(scope),
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
    /// use minitrace::{Scope, FutureExt};
    ///
    /// let (scope, _collector) = Scope::root("Task");
    ///
    /// let fut = async {
    ///     9527
    /// }.in_new_span("Future");
    ///
    /// let task = async {
    ///     fut.await
    /// };
    ///
    /// tokio::spawn(task.with_scope(scope));
    /// # }
    /// ```
    #[inline]
    fn in_new_span(self, event: &'static str) -> InNewSpan<Self> {
        InNewSpan { inner: self, event }
    }
}

#[pin_project::pin_project]
pub struct WithScope<T> {
    #[pin]
    inner: T,
    scope: Option<Scope>,
}

impl<T: std::future::Future> std::future::Future for WithScope<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let _guard = this.scope.as_ref().map(|s| s.try_enter());
        let res = this.inner.poll(cx);

        match res {
            r @ Poll::Pending => r,
            other => {
                this.scope.take();
                other
            }
        }
    }
}

#[pin_project::pin_project]
pub struct InNewSpan<T> {
    #[pin]
    inner: T,
    event: &'static str,
}

impl<T: std::future::Future> std::future::Future for InNewSpan<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = Span::enter(this.event);
        this.inner.poll(cx)
    }
}
