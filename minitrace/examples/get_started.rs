// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::collector::Config;
use minitrace::collector::TerminalReporter;
use minitrace::prelude::*;

fn main() {
    minitrace::set_reporter(TerminalReporter, Config::default());

    {
        let parent = SpanContext::new(TraceId(rand::random()), SpanId::default());
        let root = Span::root("root", parent);
        let _g = root.set_local_parent();
        let _g = LocalSpan::enter_with_local_parent("child");

        // do something ...
    }

    minitrace::flush();
}
