// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::task::Poll;

use crate::thread::AsyncGuard;

impl<T: Sized> FutureExt for T {}

pub trait FutureExt: Sized {
    #[inline]
    fn in_new_span<E: Into<u32>>(self, event: E) -> NewSpan<Self> {
        NewSpan {
            inner: self,
            event: event.into(),
            span_handle: AsyncGuard::start(false),
        }
    }
}

#[pin_project::pin_project]
pub struct NewSpan<T> {
    #[pin]
    inner: T,
    event: u32,
    span_handle: AsyncGuard,
}

impl<T: std::future::Future> std::future::Future for NewSpan<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let span_handle = std::mem::replace(this.span_handle, AsyncGuard::new_empty());

        let _guard = span_handle.ready(*this.event);
        let poll = this.inner.poll(cx);
        if poll.is_pending() {
            *this.span_handle = AsyncGuard::start(true);
        }
        poll
    }
}

impl<T: futures_01::Future> futures_01::Future for NewSpan<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let span_handle = std::mem::replace(&mut self.span_handle, AsyncGuard::new_empty());

        let _guard = span_handle.ready(self.event);
        let poll = self.inner.poll();
        if let Ok(futures_01::Async::NotReady) = poll {
            self.span_handle = AsyncGuard::start(true);
        }
        poll
    }
}
