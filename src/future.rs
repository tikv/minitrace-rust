// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::task::Poll;

use crate::thread::{new_async_scope, AsyncHandle};
use crate::trace::new_span;

impl<T: Sized> FutureExt for T {}

pub trait FutureExt: Sized {
    #[inline]
    fn in_new_scope<E: Into<u32>>(self, event: E) -> NewScope<Self> {
        NewScope {
            inner: self,
            event: event.into(),
            async_handle: new_async_scope(),
        }
    }

    #[inline]
    fn in_new_span<E: Into<u32>>(self, event: E) -> NewSpan<Self> {
        NewSpan {
            inner: self,
            event: event.into(),
        }
    }
}

#[pin_project::pin_project]
pub struct NewScope<T> {
    #[pin]
    inner: T,
    event: u32,
    async_handle: AsyncHandle,
}

impl<T: std::future::Future> std::future::Future for NewScope<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = this.async_handle.start_scope(*this.event);
        this.inner.poll(cx)
    }
}

impl<T: futures_01::Future> futures_01::Future for NewScope<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let _guard = self.async_handle.start_scope(self.event);
        self.inner.poll()
    }
}

#[pin_project::pin_project]
pub struct NewSpan<T> {
    #[pin]
    inner: T,
    event: u32,
}

impl<T: std::future::Future> std::future::Future for NewSpan<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = new_span(*this.event);
        this.inner.poll(cx)
    }
}

impl<T: futures_01::Future> futures_01::Future for NewSpan<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let _guard = new_span(self.event);
        self.inner.poll()
    }
}
