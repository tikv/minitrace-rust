// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::LocalSpanGuard;

pub struct LocalSpan;

impl LocalSpan {
    pub fn enter(event: &'static str) -> LocalSpanGuard {
        LocalSpanGuard::new(event)
    }
}
