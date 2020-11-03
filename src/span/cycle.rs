// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct Cycle(pub u64);

#[derive(Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct Realtime {
    pub epoch_time_ns: u64,
}

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
    pub realtime: Realtime,
    pub cycle: Cycle,
    pub cycles_per_second: u64,
}

impl DefaultClock {
    #[inline]
    pub fn now() -> Cycle {
        Cycle::new(minstant::now())
    }

    #[inline]
    pub fn cycle_to_realtime(cycle: Cycle, anchor: Anchor) -> Realtime {
        if cycle > anchor.cycle {
            let forward_ns = ((cycle.0 - anchor.cycle.0) as u128 * 1_000_000_000
                / anchor.cycles_per_second as u128) as u64;
            Realtime {
                epoch_time_ns: anchor.realtime.epoch_time_ns + forward_ns,
            }
        } else {
            let backward_ns = ((anchor.cycle.0 - cycle.0) as u128 * 1_000_000_000
                / anchor.cycles_per_second as u128) as u64;
            Realtime {
                epoch_time_ns: anchor.realtime.epoch_time_ns - backward_ns,
            }
        }
    }

    pub fn anchor() -> Anchor {
        let cycle = Cycle::new(minstant::now());
        let realtime = Realtime {
            epoch_time_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("unexpected time drift")
                .as_nanos() as u64,
        };
        Anchor {
            realtime,
            cycle,
            cycles_per_second: minstant::cycles_per_second(),
        }
    }
}
