// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.
//! # Get started
//!
//! 1. Setup a trace viewer/frontend. Jaeger example:
//!    ```ignore
//!    podman run -p6831:6831/udp -p6832:6832/udp -p16686:16686 jaegertracing/all-in-one:latest
//!    ```
//!
use minitrace::collector::Config;
use minitrace::collector::ConsoleReporter;
use minitrace::prelude::*;

fn main() {
    minitrace::set_reporter(ConsoleReporter, Config::default());

    {
        let parent = SpanContext::random();
        let root = Span::root("root", parent);
        let _g = root.set_local_parent();
        let _g = LocalSpan::enter_with_local_parent("child");

        // do something ...
    }

    minitrace::flush();
}
