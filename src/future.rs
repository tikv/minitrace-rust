use std::pin::Pin;
use std::task::Context;

impl<T: Sized> Instrument for T {}

pub trait Instrument: Sized {
    #[inline]
    fn instrument(self, span: crate::SpanGuard) -> Instrumented<Self> {
        Instrumented { inner: self, span }
    }

    #[inline]
    fn in_current_span<T: Into<u32>>(self, tag: T) -> Instrumented<Self> {
        Instrumented {
            inner: self,
            span: crate::new_span(tag),
        }
    }

    #[cfg(feature = "fine-async")]
    #[inline]
    fn instrument_fine(self, span: crate::SpanGuard) -> FineInstrumented<Self> {
        FineInstrumented {
            inner: self,
            context: InstrumentedContext {
                pending: Some(span),
                next_span_info: None,
            },
        }
    }

    #[cfg(feature = "fine-async")]
    #[inline]
    fn in_current_span_fine<T: Into<u32>>(self, tag: T) -> FineInstrumented<Self> {
        FineInstrumented {
            inner: self,
            context: InstrumentedContext {
                pending: Some(crate::new_span(tag)),
                next_span_info: None,
            },
        }
    }
}

#[pin_project::pin_project]
pub struct Instrumented<T> {
    #[pin]
    pub inner: T,
    pub span: crate::SpanGuard,
}

impl<T: std::future::Future> std::future::Future for Instrumented<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        let this = self.project();
        let _enter = this.span.enter();
        this.inner.poll(cx)
    }
}

impl<T: futures_01::Future> futures_01::Future for Instrumented<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let _enter = self.span.enter();
        self.inner.poll()
    }
}

#[cfg(feature = "fine-async")]
#[pin_project::pin_project]
pub struct FineInstrumented<T> {
    #[pin]
    pub inner: T,
    context: InstrumentedContext,
}

#[cfg(feature = "fine-async")]
struct InstrumentedContext {
    // trace from the Future be constructed to
    // the first time runtime polls this Future
    pending: Option<crate::SpanGuard>,

    next_span_info: Option<(
        u32,
        crate::SpanID,
        crate::time::InstantMillis,
        crate::CollectorTx,
    )>,
}

#[cfg(feature = "fine-async")]
impl InstrumentedContext {
    fn fetch_info(
        span: &crate::SpanGuard,
    ) -> Option<(
        u32,
        crate::SpanID,
        crate::time::InstantMillis,
        crate::CollectorTx,
    )> {
        span.0.as_ref().and_then(|inner| {
            inner.tx.as_ref().unwrap().try_clone().ok().map(|tx| {
                let tag = inner.info.tag;
                let id = inner.info.id;
                let root_time = inner.root_time;
                (tag, id, root_time, tx)
            })
        })
    }

    fn span(&mut self) -> crate::SpanGuard {
        let info = if self.pending.is_some() {
            // the first time be polled
            let pending = self.pending.take().unwrap();
            let info = Self::fetch_info(&pending);
            drop(pending);
            info
        } else {
            self.next_span_info.take()
        };

        crate::new_span_continue(info)
    }

    fn update_next(&mut self, span: crate::SpanGuard) {
        self.next_span_info = Self::fetch_info(&span);
    }
}

#[cfg(feature = "fine-async")]
impl<T: std::future::Future> std::future::Future for FineInstrumented<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        let this = self.project();

        let span = this.context.span();

        let output = {
            let _g = span.enter();
            this.inner.poll(cx)
        };

        if output.is_pending() {
            this.context.update_next(span);
        }

        output
    }
}

#[cfg(feature = "fine-async")]
impl<T: futures_01::Future> futures_01::Future for FineInstrumented<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> futures_01::Poll<Self::Item, Self::Error> {
        let span = self.context.span();

        let output = {
            let _g = span.enter();
            self.inner.poll()
        };

        if output
            .as_ref()
            .map(|res| res.is_not_ready())
            .unwrap_or(false)
        {
            self.context.update_next(span);
        }

        output
    }
}
