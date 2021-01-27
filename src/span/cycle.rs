// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

pub use minstant::Anchor;
pub use minstant::Cycle;

pub struct DefaultClock;

impl DefaultClock {
    #[inline]
    pub fn now() -> Cycle {
        Cycle::now()
    }

    #[inline]
    pub fn cycle_to_unix_time_ns(cycle: Cycle, anchor: Anchor) -> u64 {
        cycle.into_unix_time_ns(anchor)
    }

    #[inline]
    pub fn anchor() -> Anchor {
        Anchor::new()
    }
}
