pub use std::time::Duration;

use self::inner::monotonic_ms;
pub(crate) use self::inner::real_time_ms;

const MILLISECOND_PER_SECOND: u64 = 1_000;
const NANOSECONDS_PER_MILLISECOND: u64 = 1_000_000;

#[derive(Debug, Copy, Clone)]
pub struct InstantMillis {
    pub ms: u64,
}

impl InstantMillis {
    pub fn now() -> Self {
        monotonic_ms()
    }

    pub fn elapsed(&self) -> u32 {
        (monotonic_ms().ms - self.ms) as u32
    }
}

#[cfg(not(target_os = "linux"))]
mod inner {
    use super::{InstantMillis, MILLISECOND_PER_SECOND, NANOSECONDS_PER_MILLISECOND};

    #[inline]
    pub fn monotonic_ms() -> InstantMillis {
        let ns = time::precise_time_ns();
        let ms = ns / NANOSECONDS_PER_MILLISECOND;
        InstantMillis { ms }
    }

    #[inline]
    pub fn real_time_ms() -> u64 {
        let ts = time::get_time();
        ts.sec as u64 * MILLISECOND_PER_SECOND + ts.nsec as u64 / NANOSECONDS_PER_MILLISECOND
    }
}

#[cfg(target_os = "linux")]
mod inner {
    use super::{InstantMillis, MILLISECOND_PER_SECOND, NANOSECONDS_PER_MILLISECOND};

    pub fn monotonic_ms() -> InstantMillis {
        let mut t = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let errno = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC_COARSE, &mut t) };
        if errno != 0 {
            panic!(
                "failed to get clocktime, err {}",
                std::io::Error::last_os_error()
            );
        }

        InstantMillis {
            ms: (t.tv_sec as u64 * MILLISECOND_PER_SECOND
                + t.tv_nsec as u64 / NANOSECONDS_PER_MILLISECOND),
        }
    }

    pub fn real_time_ms() -> u64 {
        let mut t = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let errno = unsafe { libc::clock_gettime(libc::CLOCK_REALTIME_COARSE, &mut t) };
        if errno != 0 {
            panic!(
                "failed to get clocktime, err {}",
                std::io::Error::last_os_error()
            );
        }

        t.tv_sec as u64 * MILLISECOND_PER_SECOND + t.tv_nsec as u64 / NANOSECONDS_PER_MILLISECOND
    }
}
