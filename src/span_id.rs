static GLOBAL_COUNTER: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

thread_local! {
    static NEXT_LOCAL_UNIQUE_ID: std::cell::UnsafeCell<SpanID> = std::cell::UnsafeCell::new(SpanID {
        prefix: next_global(),
        offset: unsafe { std::num::NonZeroU16::new_unchecked(1) }
    })
}

fn next_global() -> u16 {
    GLOBAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SpanID {
    pub prefix: u16,
    pub offset: std::num::NonZeroU16,
}

impl SpanID {
    #[inline]
    pub fn new() -> Self {
        NEXT_LOCAL_UNIQUE_ID.with(|unique_id| unsafe {
            let next_unique_id = *unique_id.get();
            (*unique_id.get()) = if next_unique_id.offset.get() == std::u16::MAX {
                SpanID {
                    prefix: next_global(),
                    offset: std::num::NonZeroU16::new_unchecked(1),
                }
            } else {
                SpanID {
                    prefix: next_unique_id.prefix,
                    offset: std::num::NonZeroU16::new_unchecked(next_unique_id.offset.get() + 1),
                }
            };
            next_unique_id
        })
    }
}

impl Into<u32> for SpanID {
    fn into(self) -> u32 {
        unsafe { std::mem::transmute(self) }
    }
}

impl Into<std::num::NonZeroU32> for SpanID {
    fn into(self) -> std::num::NonZeroU32 {
        unsafe { std::mem::transmute(self) }
    }
}

impl std::default::Default for SpanID {
    #[inline]
    fn default() -> Self {
        SpanID::new()
    }
}
