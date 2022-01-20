// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

#[must_use]
pub struct Guard<F: FnOnce()> {
    inner: Option<F>,
}

impl<F: FnOnce()> Guard<F> {
    pub fn new(f: F) -> Self {
        Self { inner: Some(f) }
    }
}

impl<F: FnOnce()> Drop for Guard<F> {
    fn drop(&mut self) {
        if let Some(f) = self.inner.take() {
            f()
        }
    }
}
