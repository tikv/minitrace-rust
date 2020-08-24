// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::task::Poll;

use crate::collector::Collector;
use crate::thread::{new_async_scope, AsyncScopeHandle};

impl<T: Sized> FutureExt for T {}

pub trait FutureExt: Sized {
    #[inline]
    fn in_new_span<E: Into<u32>>(self, event: E) -> NewSpan<Self> {
        let event = event.into();
        NewSpan {
            inner: self,
            event,
            trace_handle: new_async_scope(),
        }
    }

    #[inline]
    fn collect_trace<E: Into<u32>>(self, event: E) -> CollectTrace<Self> {
        let event = event.into();
        let collector = Collector::new();

        CollectTrace {
            inner: self,
            event,
            trace_handle: AsyncScopeHandle::new_root(collector.inner.clone()),
            collector: Some(collector),
        }
    }
}

#[pin_project::pin_project]
pub struct NewSpan<T> {
    #[pin]
    inner: T,
    event: u32,
    trace_handle: AsyncScopeHandle,
}

impl<T: std::future::Future> std::future::Future for NewSpan<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = this.trace_handle.start_trace(*this.event);
        this.inner.poll(cx)
    }
}

impl<T: futures_01::Future> futures_01::Future for NewSpan<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let _guard = self.trace_handle.start_trace(self.event);
        self.inner.poll()
    }
}

#[pin_project::pin_project]
pub struct CollectTrace<T> {
    #[pin]
    inner: T,
    event: u32,
    collector: Option<crate::collector::Collector>,
    trace_handle: AsyncScopeHandle,
}

impl<T: std::future::Future> std::future::Future for CollectTrace<T> {
    type Output = (Collector, T::Output);

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let guard = this.trace_handle.start_trace(*this.event);
        let r = this.inner.poll(cx);

        let r = match r {
            Poll::Ready(r) => r,
            Poll::Pending => return Poll::Pending,
        };

        drop(guard);

        // mute rust-analyzer
        let oc: &mut Option<_> = this.collector;
        Poll::Ready((oc.take().expect("poll twice"), r))
    }
}

impl<T: futures_01::Future> futures_01::Future for CollectTrace<T> {
    type Item = (Collector, T::Item);
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let guard = self.trace_handle.start_trace(self.event);
        let r = self.inner.poll();

        let r = match r {
            Err(r) => {
                let _ = self.collector.take();
                return Err(r);
            }
            Ok(futures_01::Async::Ready(r)) => r,
            Ok(_) => {
                return Ok(futures_01::Async::NotReady);
            }
        };

        drop(guard);
        Ok(futures_01::Async::Ready((
            self.collector.take().expect("poll twice"),
            r,
        )))
    }
}
