// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use parking_lot::Mutex;
use std::sync::Arc;

pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let page = Arc::new(Mutex::new(Vec::new()));
    (
        Sender { page: page.clone() },
        Receiver {
            page,
            received: Vec::new(),
        },
    )
}

pub struct Sender<T> {
    page: Arc<Mutex<Vec<T>>>,
}

pub struct Receiver<T> {
    page: Arc<Mutex<Vec<T>>>,
    received: Vec<T>,
}

#[derive(Debug)]
pub struct ChannelClosed;

impl<T> Sender<T> {
    pub fn send(&self, value: T) {
        let mut page = self.page.lock();
        page.push(value);
    }
}

impl<T> Receiver<T> {
    pub fn try_recv(&mut self) -> Result<Option<T>, ChannelClosed> {
        match self.received.pop() {
            Some(val) => Ok(Some(val)),
            None => {
                let is_disconnected = Arc::strong_count(&self.page) < 2;
                {
                    let mut page = self.page.lock();
                    std::mem::swap(&mut *page, &mut self.received);
                }
                match self.received.pop() {
                    Some(val) => Ok(Some(val)),
                    None if is_disconnected => Err(ChannelClosed),
                    None => Ok(None),
                }
            }
        }
    }
}
