// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use parking_lot::Mutex;
use std::ops::{Deref, DerefMut};

pub struct Pool<T> {
    objects: Mutex<Vec<T>>,
    init: fn() -> T,
    reset: fn(&mut T),
}

impl<T> Pool<T> {
    #[inline]
    pub fn new(init: fn() -> T, reset: fn(&mut T)) -> Pool<T> {
        Pool {
            objects: Mutex::new(Vec::new()),
            init,
            reset,
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn pull(&self) -> Reusable<T> {
        self.objects
            .lock()
            .pop()
            .map(|obj| Reusable::new(self, obj))
            .unwrap_or_else(|| Reusable::new(self, (self.init)()))
    }

    #[inline]
    pub fn batch_pull<'a>(&'a self, n: usize, buffer: &mut Vec<Reusable<'a, T>>) {
        let mut objects = self.objects.lock();
        let len = objects.len();
        buffer.extend(
            objects
                .drain(len.saturating_sub(n)..)
                .chain(std::iter::repeat_with(self.init))
                .take(n)
                .map(|obj| Reusable::new(self, obj)),
        );
    }

    pub fn puller(&self, buffer_size: usize) -> Puller<T> {
        assert!(buffer_size > 0);
        Puller {
            pool: self,
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
        }
    }

    #[inline]
    pub fn recycle(&self, mut obj: T) {
        (self.reset)(&mut obj);
        self.objects.lock().push(obj)
    }
}

pub struct Puller<'a, T> {
    pool: &'a Pool<T>,
    buffer: Vec<Reusable<'a, T>>,
    buffer_size: usize,
}

impl<'a, T> Puller<'a, T> {
    #[inline]
    pub fn pull(&mut self) -> Reusable<'a, T> {
        self.buffer.pop().unwrap_or_else(|| {
            self.pool.batch_pull(self.buffer_size, &mut self.buffer);
            self.buffer.pop().unwrap()
        })
    }
}

pub struct Reusable<'a, T> {
    pool: &'a Pool<T>,
    obj: Option<T>,
}

impl<'a, T> Reusable<'a, T> {
    #[inline]
    pub fn new(pool: &'a Pool<T>, t: T) -> Self {
        Self { pool, obj: Some(t) }
    }

    #[inline]
    pub fn into_inner(mut self) -> (&'a Pool<T>, T) {
        (self.pool, self.obj.take().unwrap())
    }
}

impl<'a, T> std::fmt::Debug for Reusable<'a, T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.obj.as_ref().unwrap().fmt(f)
    }
}

impl<'a, T> Deref for Reusable<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.obj.as_ref().unwrap()
    }
}

impl<'a, T> DerefMut for Reusable<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.obj.as_mut().unwrap()
    }
}

impl<'a, T> Drop for Reusable<'a, T> {
    #[inline]
    fn drop(&mut self) {
        if let Some(obj) = self.obj.take() {
            self.pool.recycle(obj);
        }
    }
}
