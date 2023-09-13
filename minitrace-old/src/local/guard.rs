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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn guard_basic() {
        let a = Cell::new(0);
        {
            let _guard = Guard::new(|| a.set(1));
            assert_eq!(a.get(), 0);
        }
        assert_eq!(a.get(), 1);
    }
}
