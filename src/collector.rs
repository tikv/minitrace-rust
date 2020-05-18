pub struct Collector;

impl Collector {
    pub fn bounded(capacity: u16) -> (CollectorTx, CollectorRx) {
        assert!(capacity > 0);

        let (tx, rx) = ArrayCollector::bounded(capacity);
        (CollectorTx::Array(tx), CollectorRx::Array(rx))
    }

    pub fn unbounded() -> (CollectorTx, CollectorRx) {
        let (tx, rx) = crossbeam::channel::unbounded();
        (CollectorTx::Channel(tx), CollectorRx::Channel(rx))
    }

    pub fn void() -> (CollectorTx, CollectorRx) {
        (CollectorTx::Void, CollectorRx::Void)
    }
}

pub enum CollectorTx {
    Void,
    Channel(crossbeam::channel::Sender<crate::Span>),
    Array(ArrayCollectorTx),
}

impl CollectorTx {
    #[inline]
    pub fn put(self, span: crate::Span) {
        match self {
            CollectorTx::Void => (),
            CollectorTx::Channel(c) => {
                let _ = c.try_send(span);
            }
            CollectorTx::Array(a) => a.put(span),
        }
    }

    pub fn try_clone(&self) -> Result<Self, Box<dyn std::error::Error>> {
        match self {
            CollectorTx::Void => Ok(CollectorTx::Void),
            CollectorTx::Channel(c) => Ok(CollectorTx::Channel(c.clone())),
            CollectorTx::Array(a) => Ok(CollectorTx::Array(a.try_clone()?)),
        }
    }
}

pub enum CollectorRx {
    Void,
    Channel(crossbeam::channel::Receiver<crate::Span>),
    Array(ArrayCollectorRx),
}

impl CollectorRx {
    /// If there are `SpanGuard`s have not been dropped,
    /// `Channel` will block until all of them are dropped and
    /// `Array` will return an error.
    #[inline]
    pub fn collect(&mut self) -> Result<Vec<crate::Span>, Box<dyn std::error::Error>> {
        match self {
            CollectorRx::Void => Ok(vec![]),
            CollectorRx::Channel(c) => Ok(c.iter().collect()),
            CollectorRx::Array(a) => a.collect(),
        }
    }

    #[inline]
    pub fn try_collect(&self) -> Vec<crate::Span> {
        match self {
            CollectorRx::Void => vec![],
            CollectorRx::Channel(c) => c.try_iter().collect(),
            CollectorRx::Array(_) => vec![],
        }
    }
}

struct ArrayCollector {
    /// An array of `Span`
    ///
    /// Each element is owned by individual `ArrayCollectorTx`,
    /// so there is no shared mutable data.
    buffer: *mut crate::Span,

    /// Shared record
    ///
    /// Leftmost 16 bits as reference count of TXs,
    /// Rightmost 16 bits as total count of TXs, id est, count of Spans.
    record: std::sync::atomic::AtomicU32,

    capacity: u16,
}

impl ArrayCollector {
    fn bounded(capacity: u16) -> (ArrayCollectorTx, ArrayCollectorRx) {
        let buffer = {
            let mut v = Vec::<crate::Span>::with_capacity(capacity as usize);
            let ptr = v.as_mut_ptr();
            std::mem::forget(v);
            ptr
        };

        let mut collector = Box::new(Self {
            buffer,
            // reference count: 1, total count: 1
            record: std::sync::atomic::AtomicU32::new(1 + (1 << 16)),
            capacity,
        });

        (
            ArrayCollectorTx {
                collector: collector.as_mut() as *mut _,
                index: 0,
                capacity,
            },
            ArrayCollectorRx {
                collector: Some(collector),
            },
        )
    }
}

impl Drop for ArrayCollector {
    fn drop(&mut self) {
        let record = self.record.load(std::sync::atomic::Ordering::SeqCst);

        // reference count >= 1
        if record >= (1 << 16) {
            panic!("exists spans alive");
        }

        // deallocate the array
        unsafe {
            Vec::from_raw_parts(self.buffer, 0, self.capacity as usize);
        }
    }
}

pub struct ArrayCollectorTx {
    /// Shared Collector
    ///
    /// Fields of this collector are immutable once it was constructed.
    collector: *mut ArrayCollector,

    /// Index of Vec to put Span
    index: u16,

    capacity: u16,
}

impl ArrayCollectorTx {
    fn put(self, span: crate::Span) {
        unsafe {
            let c = &*self.collector;
            let slot = c.buffer.add(self.index as usize);
            std::ptr::write(slot, span)
        }
    }

    fn try_clone(&self) -> Result<Self, Box<dyn std::error::Error>> {
        let collect = unsafe { &*self.collector };

        let r: Result<_, Box<dyn std::error::Error>> = collect
            .record
            .fetch_update(
                |x| {
                    if x as u16 >= self.capacity {
                        // index out of range
                        None
                    } else {
                        // total count + 1, reference count + 1
                        Some(x + 1 + (1 << 16))
                    }
                },
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .map_err(|_| "too many spans".into());

        let index = r? as u16;

        Ok(Self {
            collector: self.collector,
            index,
            capacity: self.capacity,
        })
    }
}

impl Drop for ArrayCollectorTx {
    fn drop(&mut self) {
        let collect = unsafe { &*self.collector };
        collect
            .record
            .fetch_update(
                |x| {
                    // reference count - 1
                    Some(x - (1 << 16))
                },
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .unwrap();
    }
}

pub struct ArrayCollectorRx {
    collector: Option<Box<ArrayCollector>>,
}

impl ArrayCollectorRx {
    fn collect(&mut self) -> Result<Vec<crate::Span>, Box<dyn std::error::Error>> {
        if self.collector.is_none() {
            return Err("already been collected".into());
        }

        let record = self
            .collector
            .as_ref()
            .unwrap()
            .record
            .load(std::sync::atomic::Ordering::SeqCst);

        // reference count >= 1
        if record >= (1 << 16) {
            return Err("exists unfinished spans".into());
        }

        let len = record as u16;

        let mut collector = self.collector.take().unwrap();
        let buffer = collector.buffer;
        let capacity = collector.capacity;
        unsafe {
            std::alloc::dealloc(
                collector.as_mut() as *mut ArrayCollector as *mut u8,
                std::alloc::Layout::new::<ArrayCollector>(),
            );

            // If we want to reuse the array to return `Vec` and reduce memory allocation,
            // we have to provent `collector` from deallocating the array.
            std::mem::forget(collector);
            Ok(Vec::from_raw_parts(buffer, len as usize, capacity as usize))
        }
    }
}

unsafe impl Send for ArrayCollectorTx {}
unsafe impl Send for ArrayCollectorRx {}
