// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::{LocalSpanGuard, Span};

impl Span {
    pub fn start(event: &'static str) -> LocalSpanGuard {
        LocalSpanGuard::start(event)
    }
}
