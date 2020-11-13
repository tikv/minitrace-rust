// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::task::Poll;

use crate::{start_scope, start_span, Scope};

impl<T: Sized> FutureExt for T {}

pub trait FutureExt: Sized {
    #[inline]
    fn with_scope(self, scope: Scope) -> WithScope<Self> {
        WithScope { inner: self, scope }
    }

    #[inline]
    fn in_new_span(self, event: &'static str) -> WithSpan<Self> {
        WithSpan { inner: self, event }
    }
}

#[pin_project::pin_project]
pub struct WithScope<T> {
    #[pin]
    inner: T,
    scope: Scope,
}

impl<T: std::future::Future> std::future::Future for WithScope<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = start_scope(this.scope);
        match this.inner.poll(cx) {
            r @ Poll::Pending => r,
            other => {
                this.scope.release();
                other
            }
        }
    }
}

impl<T: futures_01::Future> futures_01::Future for WithScope<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let _guard = start_scope(&self.scope);
        match self.inner.poll() {
            r @ Ok(futures_01::Async::NotReady) => r,
            other => {
                self.scope.release();
                other
            }
        }
    }
}

#[pin_project::pin_project]
pub struct WithSpan<T> {
    #[pin]
    inner: T,
    event: &'static str,
}

impl<T: std::future::Future> std::future::Future for WithSpan<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = start_span(this.event);
        this.inner.poll(cx)
    }
}

impl<T: futures_01::Future> futures_01::Future for WithSpan<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let _guard = start_span(self.event);
        self.inner.poll()
    }
}
