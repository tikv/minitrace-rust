// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

impl<T: Sized> Instrument for T {}
pub trait Instrument: Sized {
    #[inline]
    fn trace_task<T: Into<u32>>(self, event: T) -> TraceSpawned<Self> {
        TraceSpawned {
            inner: self,
            event: event.into(),
            crossthread_trace: crate::trace::trace_crossthread(),
        }
    }

    #[inline]
    fn trace_async<T: Into<u32>>(self, event: T) -> TraceWrapped<Self> {
        TraceWrapped {
            inner: self,
            event: event.into(),
        }
    }

    #[inline]
    fn future_trace_enable<T: Into<u32>>(self, event: T) -> TraceRootFuture<Self> {
        let now = crate::time::real_time_ns();
        let collector = crate::collector::Collector::new(now);

        TraceRootFuture {
            inner: self,
            event: event.into(),
            crossthread_trace: crate::trace_crossthread::CrossthreadTrace::new_root(
                now,
                collector.inner.clone(),
            ),
            collector: Some(collector),
        }
    }

    #[inline]
    fn future_trace_may_enable<T: Into<u32>>(
        self,
        enable: bool,
        event: T,
    ) -> MayTraceRootFuture<Self> {
        if enable {
            let now = crate::time::real_time_ns();
            let collector = crate::collector::Collector::new(now);
            MayTraceRootFuture {
                inner: self,
                event: event.into(),
                crossthread_trace: Some(crate::trace_crossthread::CrossthreadTrace::new_root(
                    now,
                    collector.inner.clone(),
                )),
                collector: Some(collector),
            }
        } else {
            MayTraceRootFuture {
                inner: self,
                event: event.into(),
                collector: None,
                crossthread_trace: None,
            }
        }
    }
}

#[pin_project::pin_project]
pub struct TraceSpawned<T> {
    #[pin]
    inner: T,
    event: u32,
    crossthread_trace: crate::trace_crossthread::CrossthreadTrace,
}

impl<T: std::future::Future> std::future::Future for TraceSpawned<T> {
    type Output = T::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        let _guard = this.crossthread_trace.trace_enable(*this.event);
        this.inner.poll(cx)
    }
}

impl<T: futures_01::Future> futures_01::Future for TraceSpawned<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let _guard = self.crossthread_trace.trace_enable(self.event);
        self.inner.poll()
    }
}

#[pin_project::pin_project]
pub struct TraceWrapped<T> {
    #[pin]
    inner: T,
    event: u32,
}

impl<T: std::future::Future> std::future::Future for TraceWrapped<T> {
    type Output = T::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        let _guard = crate::trace::new_span(*this.event);
        this.inner.poll(cx)
    }
}

impl<T: futures_01::Future> futures_01::Future for TraceWrapped<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let _guard = crate::trace::new_span(self.event);
        self.inner.poll()
    }
}

#[pin_project::pin_project]
pub struct MayTraceRootFuture<T> {
    #[pin]
    inner: T,
    event: u32,
    collector: Option<crate::collector::Collector>,
    crossthread_trace: Option<crate::trace_crossthread::CrossthreadTrace>,
}

impl<T: std::future::Future> std::future::Future for MayTraceRootFuture<T> {
    type Output = (Option<crate::TraceDetails>, T::Output);

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        let event = *this.event;
        let guard = this
            .crossthread_trace
            .as_mut()
            .and_then(|a| a.trace_enable(event));
        let r = this.inner.poll(cx);

        let r = match r {
            std::task::Poll::Ready(r) => r,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        };

        drop(guard);
        std::task::Poll::Ready((this.collector.take().map(|c| c.collect()), r))
    }
}

impl<T: futures_01::Future> futures_01::Future for MayTraceRootFuture<T> {
    type Item = (Option<crate::TraceDetails>, T::Item);
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let event = self.event;
        let guard = self
            .crossthread_trace
            .as_mut()
            .and_then(|a| a.trace_enable(event));
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
            self.collector.take().map(|c| c.collect()),
            r,
        )))
    }
}

#[pin_project::pin_project]
pub struct TraceRootFuture<T> {
    #[pin]
    inner: T,
    event: u32,
    collector: Option<crate::collector::Collector>,
    crossthread_trace: crate::trace_crossthread::CrossthreadTrace,
}

impl<T: std::future::Future> std::future::Future for TraceRootFuture<T> {
    type Output = (crate::TraceDetails, T::Output);

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        let guard = this.crossthread_trace.trace_enable(*this.event);
        let r = this.inner.poll(cx);

        let r = match r {
            std::task::Poll::Ready(r) => r,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        };

        drop(guard);
        std::task::Poll::Ready((this.collector.take().expect("poll twice").collect(), r))
    }
}

impl<T: futures_01::Future> futures_01::Future for TraceRootFuture<T> {
    type Item = (crate::TraceDetails, T::Item);
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let guard = self.crossthread_trace.trace_enable(self.event);
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
            self.collector.take().expect("poll twice").collect(),
            r,
        )))
    }
}
