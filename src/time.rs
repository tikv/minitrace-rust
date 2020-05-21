const MILLISECOND_PER_SECOND: u64 = 1_000;
const NANOSECONDS_PER_MILLISECOND: u64 = 1_000_000;

#[derive(Debug, Copy, Clone)]
pub struct InstantMillis {
    pub ms: u64,
}

impl InstantMillis {
    #[cfg(not(target_os = "linux"))]
    #[inline]
    pub fn now() -> Self {
        let ns = time::precise_time_ns();
        let ms = ns / NANOSECONDS_PER_MILLISECOND;
        InstantMillis { ms }
    }

    #[cfg(target_os = "linux")]
    #[inline]
    pub fn now() -> Self {
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
            ms: (t.tv_sec as u64)
                .wrapping_mul(MILLISECOND_PER_SECOND)
                .wrapping_add((t.tv_nsec as u64) / NANOSECONDS_PER_MILLISECOND),
        }
    }

    #[inline]
    pub fn elapsed_ms(&self) -> u32 {
        Self::now().ms.wrapping_sub(self.ms) as u32
    }
}

#[cfg(not(target_os = "linux"))]
#[inline]
pub fn real_time_ms() -> u64 {
    let t = time::get_time();
    (t.sec as u64)
        .wrapping_mul(MILLISECOND_PER_SECOND)
        .wrapping_add((t.nsec as u64) / NANOSECONDS_PER_MILLISECOND)
}

#[cfg(target_os = "linux")]
#[inline]
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

    (t.tv_sec as u64)
        .wrapping_mul(MILLISECOND_PER_SECOND)
        .wrapping_add((t.tv_nsec as u64) / NANOSECONDS_PER_MILLISECOND)
}
