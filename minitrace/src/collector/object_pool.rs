// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use parking_lot::Mutex;
use std::mem::{forget, ManuallyDrop};
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
    pub fn len(&self) -> usize {
        self.objects.lock().len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.objects.lock().is_empty()
    }

    #[inline]
    pub fn pull(&self) -> Reusable<T> {
        self.objects
            .lock()
            .pop()
            .map(|mut obj| {
                (self.reset)(&mut obj);
                Reusable::new(self, obj)
            })
            .unwrap_or_else(|| Reusable::new(self, (self.init)()))
    }

    #[inline]
    pub fn batch_pull<'a>(&'a self, n: usize, buffer: &mut Vec<Reusable<'a, T>>) {
        let mut objects = self.objects.lock();
        let len = objects.len();
        buffer.extend(
            objects
                .drain(len.saturating_sub(n)..)
                .map(|mut obj| {
                    (self.reset)(&mut obj);
                    obj
                })
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
    pub fn recycle(&self, t: T) {
        self.objects.lock().push(t)
    }
}

pub struct Puller<'a, T> {
    pool: &'a Pool<T>,
    buffer: Vec<Reusable<'a, T>>,
    buffer_size: usize,
}

impl<'a, T> Puller<'a, T> {
    #[inline]
    pub fn pull(&mut self) -> Reusable<T> {
        self.buffer.pop().unwrap_or_else(|| {
            self.pool.batch_pull(self.buffer_size, &mut self.buffer);
            self.buffer.pop().unwrap()
        })
    }
}

pub struct Reusable<'a, T> {
    pool: &'a Pool<T>,
    obj: ManuallyDrop<T>,
}

impl<'a, T> Reusable<'a, T> {
    #[inline]
    pub fn new(pool: &'a Pool<T>, t: T) -> Self {
        Self {
            pool,
            obj: ManuallyDrop::new(t),
        }
    }

    #[inline]
    pub fn into_inner(mut self) -> (&'a Pool<T>, T) {
        let ret = unsafe { (self.pool, self.take()) };
        forget(self);
        ret
    }

    unsafe fn take(&mut self) -> T {
        ManuallyDrop::take(&mut self.obj)
    }
}

impl<'a, T> Deref for Reusable<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl<'a, T> DerefMut for Reusable<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.obj
    }
}

impl<'a, T> Drop for Reusable<'a, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe { self.pool.recycle(self.take()) }
    }
}
