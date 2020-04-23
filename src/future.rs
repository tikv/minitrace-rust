#[pin_project::pin_project]
pub struct Instrumented<T> {
    #[pin]
    inner: T,
    span: crate::SpanGuard,
}

impl<T: std::future::Future + Sized> Instrument for T {}

pub trait Instrument: std::future::Future + Sized {
    fn instrument(self, span: crate::SpanGuard) -> Instrumented<Self> {
        Instrumented { inner: self, span }
    }

    fn in_current_span(self) -> Instrumented<Self> {
        Instrumented {
            inner: self,
            span: crate::new_span(),
        }
    }
}

impl<T: std::future::Future> std::future::Future for Instrumented<T> {
    type Output = T::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        let _enter = this.span.enter();
        this.inner.poll(cx)
    }
}
