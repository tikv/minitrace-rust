// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

pub(crate) struct CollectorInner {
    pub(crate) queue: crossbeam::queue::SegQueue<crate::SpanSet>,
    pub(crate) closed: std::sync::atomic::AtomicBool,
}

pub struct Collector {
    pub(crate) inner: std::sync::Arc<CollectorInner>,
}

impl Collector {
    pub(crate) fn new() -> Self {
        let collector = std::sync::Arc::new(crate::collector::CollectorInner {
            queue: crossbeam::queue::SegQueue::new(),
            closed: std::sync::atomic::AtomicBool::new(false),
        });

        crate::collector::Collector { inner: collector }
    }

    #[inline]
    pub fn collect(self) -> Vec<crate::SpanSet> {
        self.collect_once()
    }

    #[inline]
    pub fn collect_once(&self) -> Vec<crate::SpanSet> {
        let len = self.inner.queue.len();
        let mut res = Vec::with_capacity(len);
        while let Ok(spans) = self.inner.queue.pop() {
            res.push(spans);
        }
        res
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        self.inner
            .closed
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
