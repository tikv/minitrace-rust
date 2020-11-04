// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::BTreeSet;

#[derive(Default)]
pub struct Registry {
    listeners: BTreeSet<Listener>,
}

impl Registry {
    pub fn register(&mut self, listener: Listener) {
        self.listeners.insert(listener);
    }

    pub fn unregister(&mut self, listener: Listener) {
        self.listeners.remove(&listener);
    }

    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }

    pub fn oldest_listener(&self) -> Option<Listener> {
        self.listeners.first().cloned()
    }

    pub fn len(&self) -> usize {
        self.listeners.len()
    }
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct Listener {
    pub(super) queue_index: usize,
    pub(super) slab_index: usize,
}

impl Listener {
    pub fn new(queue_index: usize, slab_index: usize) -> Self {
        Listener {
            queue_index,
            slab_index,
        }
    }
}
