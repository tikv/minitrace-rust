// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::local::LocalSpanGuard;

pub struct LocalSpan;

impl LocalSpan {
    pub fn enter_with_local_parent(event: &'static str) -> LocalSpanGuard {
        LocalSpanGuard::new(event)
    }
}
