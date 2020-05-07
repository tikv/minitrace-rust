use time::Timespec;
pub use std::time::Duration;

use self::inner::monotonic_coarse_now;
pub use self::inner::monotonic_now;
pub use self::inner::monotonic_raw_now;

const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;
const MILLISECOND_PER_SECOND: i64 = 1_000;
const NANOSECONDS_PER_MILLISECOND: i64 = 1_000_000;

#[cfg(not(target_os = "linux"))]
mod inner {
    use super::NANOSECONDS_PER_SECOND;
    use time::{self, Timespec};

    pub fn monotonic_raw_now() -> Timespec {
        let ns = time::precise_time_ns();
        let s = ns / NANOSECONDS_PER_SECOND;
        let ns = ns % NANOSECONDS_PER_SECOND;
        Timespec::new(s as i64, ns as i32)
    }

    pub fn monotonic_now() -> Timespec {
        monotonic_raw_now()
    }

    pub fn monotonic_coarse_now() -> Timespec {
        monotonic_raw_now()
    }
}

#[cfg(target_os = "linux")]
mod inner {
    use std::io;
    use time::Timespec;

    #[inline]
    pub fn monotonic_raw_now() -> Timespec {
        get_time(libc::CLOCK_MONOTONIC_RAW)
    }

    #[inline]
    pub fn monotonic_now() -> Timespec {
        get_time(libc::CLOCK_MONOTONIC)
    }

    #[inline]
    pub fn monotonic_coarse_now() -> Timespec {
        get_time(libc::CLOCK_MONOTONIC_COARSE)
    }

    #[inline]
    fn get_time(clock: libc::clockid_t) -> Timespec {
        let mut t = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let errno = unsafe { libc::clock_gettime(clock, &mut t) };
        if errno != 0 {
            panic!(
                "failed to get clocktime, err {}",
                io::Error::last_os_error()
            );
        }
        Timespec::new(t.tv_sec, t.tv_nsec as _)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Instant {
    Monotonic(Timespec),
    MonotonicCoarse(Timespec),
}

impl Instant {
    pub fn now() -> Instant {
        Instant::Monotonic(monotonic_now())
    }

    pub fn now_coarse() -> Instant {
        Instant::MonotonicCoarse(monotonic_coarse_now())
    }

    pub fn elapsed(&self) -> Duration {
        match *self {
            Instant::Monotonic(t) => {
                let now = monotonic_now();
                Instant::elapsed_duration(now, t)
            }
            Instant::MonotonicCoarse(t) => {
                let now = monotonic_coarse_now();
                Instant::elapsed_duration_coarse(now, t)
            }
        }
    }

    pub fn duration_since(&self, earlier: Instant) -> Duration {
        match (*self, earlier) {
            (Instant::Monotonic(later), Instant::Monotonic(earlier)) => {
                Instant::elapsed_duration(later, earlier)
            }
            (Instant::MonotonicCoarse(later), Instant::MonotonicCoarse(earlier)) => {
                Instant::elapsed_duration_coarse(later, earlier)
            }
            _ => {
                panic!("duration between different types of Instants");
            }
        }
    }

    pub fn elapsed_duration(later: Timespec, earlier: Timespec) -> Duration {
        if later >= earlier {
            (later - earlier).to_std().unwrap()
        } else {
            panic!(
                "monotonic time jumped back, {:.9} -> {:.9}",
                earlier.sec as f64 + f64::from(earlier.nsec) / NANOSECONDS_PER_SECOND as f64,
                later.sec as f64 + f64::from(later.nsec) / NANOSECONDS_PER_SECOND as f64
            );
        }
    }

    fn elapsed_duration_coarse(later: Timespec, earlier: Timespec) -> Duration {
        let later_ms = later.sec * MILLISECOND_PER_SECOND
            + i64::from(later.nsec) / NANOSECONDS_PER_MILLISECOND;
        let earlier_ms = earlier.sec * MILLISECOND_PER_SECOND
            + i64::from(earlier.nsec) / NANOSECONDS_PER_MILLISECOND;
        let dur = later_ms - earlier_ms;
        if dur >= 0 {
            Duration::from_millis(dur as u64)
        } else {
            Duration::from_millis(0)
        }
    }
}

#[inline]
pub fn duration_to_ms(d: Duration) -> u32 {
    d.as_secs() as u32 * 1_000 + (d.subsec_nanos() / 1_000_000)
}
