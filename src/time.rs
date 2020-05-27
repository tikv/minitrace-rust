#[cfg(target_arch = "x86")]
use core::arch::x86::_rdtsc;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::_rdtsc;

lazy_static::lazy_static! {
    static ref CYCLES_PER_SEC: u64 = init_cycles_per_sec();
    static ref OFFSET_INSTANT: std::time::Instant = std::time::Instant::now();
}

#[inline]
pub(crate) fn real_time_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .expect("SystemTime before UNIX EPOCH!")
        .as_nanos() as u64
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
pub(crate) fn monotonic_cycles() -> u64 {
    unsafe { _rdtsc() }
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline]
pub(crate) fn monotonic_cycles() -> u64 {
    (*OFFSET_INSTANT).elapsed().as_nanos() as u64
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
pub fn cycles_per_sec() -> u64 {
    *CYCLES_PER_SEC
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline]
pub fn cycles_per_sec() -> u64 {
    1_000_000_000
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn init_cycles_per_sec() -> u64 {
    // Compute the frequency of the fine-grained CPU timer: to do this,
    // take parallel time readings using both rdtsc and gettimeofday.
    // After 10ms have elapsed, take the ratio between these readings.

    let mut cycles_per_sec;
    let mut old_cycles = 0.0;

    // There is one tricky aspect, which is that we could get interrupted
    // between calling gettimeofday and reading the cycle counter, in which
    // case we won't have corresponding readings.  To handle this (unlikely)
    // case, compute the overall result repeatedly, and wait until we get
    // two successive calculations that are within 0.001% of each other (or
    // in other words, a drift of up to 10µs per second).
    loop {
        let time1 = std::time::Instant::now();
        let start_cycles = unsafe { _rdtsc() };
        loop {
            let duration = time1.elapsed();
            let stop_cycles = unsafe { _rdtsc() };
            let micros = duration.as_micros();
            if micros > 10_000 {
                cycles_per_sec = 1_000_000.0 * (stop_cycles - start_cycles) as f64 / micros as f64;
                break;
            }
        }
        let delta = cycles_per_sec / 100_000.0;
        if old_cycles > (cycles_per_sec - delta) && old_cycles < (cycles_per_sec + delta) {
            break;
        }
        old_cycles = cycles_per_sec;
    }

    cycles_per_sec.round() as u64
}
