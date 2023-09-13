// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright 2022 Tokio project authors

use minitrace::trace;

// Reproduces https://github.com/tokio-rs/tracing/issues/1613
// and https://github.com/rust-lang/rust-clippy/issues/7760
#[trace]
#[deny(clippy::suspicious_else_formatting)]
async fn re_a() {
    // hello world
    // else
}

// Reproduces https://github.com/tokio-rs/tracing/issues/1613
#[trace]
// LOAD-BEARING `#[rustfmt::skip]`! This is necessary to reproduce the bug;
// with the rustfmt-generated formatting, the lint will not be triggered!
#[rustfmt::skip]
#[deny(clippy::suspicious_else_formatting)]
async fn re_b(var: bool) {
    println!(
        "{}",
        if var { "true" } else { "false" }
    );
}