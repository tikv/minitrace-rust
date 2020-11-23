// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct Cycle(pub u64);

impl Cycle {
    pub fn new(cycles: u64) -> Self {
        Cycle(cycles)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

pub struct DefaultClock;

#[derive(Copy, Clone, Default)]
pub struct Anchor {
    pub unix_time_ns: u64,
    pub cycle: Cycle,
    pub cycles_per_second: u64,
}

impl DefaultClock {
    #[inline]
    pub fn now() -> Cycle {
        Cycle::new(minstant::now())
    }

    #[inline]
    pub fn cycle_to_unix_time_ns(cycle: Cycle, anchor: Anchor) -> u64 {
        if cycle > anchor.cycle {
            let forward_ns = ((cycle.0 - anchor.cycle.0) as f64 * 1_000_000_000.0
                / anchor.cycles_per_second as f64) as u64;
            anchor.unix_time_ns + forward_ns
        } else {
            let backward_ns = ((anchor.cycle.0 - cycle.0) as f64 * 1_000_000_000.0
                / anchor.cycles_per_second as f64) as u64;
            anchor.unix_time_ns - backward_ns
        }
    }

    pub fn anchor() -> Anchor {
        let cycle = Cycle::new(minstant::now());
        let unix_time_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unexpected time drift")
            .as_nanos() as u64;
        Anchor {
            unix_time_ns,
            cycle,
            cycles_per_second: minstant::cycles_per_second(),
        }
    }
}
