// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#[inline(always)]
pub fn real_time_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .expect("SystemTime before UNIX EPOCH!")
        .as_nanos() as u64
}
