impl<T: Sized> Instrument for T {}
pub trait Instrument: Sized {
    #[inline]
    fn trace_task<T: Into<u32>>(self, event: T) -> TraceSpawned<Self> {
        TraceSpawned {
            inner: self,
            crossthread_trace: crate::trace::trace_crossthread(event),
        }
    }

    #[inline]
    fn trace_async<T: Into<u32>>(self, event: T) -> TraceWrapped<Self> {
        TraceWrapped {
            inner: self,
            event: event.into(),
        }
    }
}

#[pin_project::pin_project]
pub struct TraceSpawned<T> {
    #[pin]
    inner: T,
    crossthread_trace: crate::trace_crossthread::CrossthreadTrace,
}

impl<T: std::future::Future> std::future::Future for TraceSpawned<T> {
    type Output = T::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        let _guard = this.crossthread_trace.trace_enable();
        this.inner.poll(cx)
    }
}

impl<T: futures_01::Future> futures_01::Future for TraceSpawned<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let _guard = self.crossthread_trace.trace_enable();
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
