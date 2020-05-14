pub use std::time::Duration;

use self::inner::monotonic_ms;

const MILLISECOND_PER_SECOND: i64 = 1_000;
const NANOSECONDS_PER_MILLISECOND: i64 = 1_000_000;

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
    use super::{InstantMillis, NANOSECONDS_PER_MILLISECOND};

    #[inline]
    pub fn monotonic_ms() -> InstantMillis {
        let ns = time::precise_time_ns();
        let ms = ns / NANOSECONDS_PER_MILLISECOND;
        InstantMillis { ms }
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
            ms: (t.tv_sec * MILLISECOND_PER_SECOND + t.tv_nsec / NANOSECONDS_PER_MILLISECOND)
                as u64,
        }
    }
}
