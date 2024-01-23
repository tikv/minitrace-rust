// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use minitrace::collector::Config;
use minitrace::collector::ConsoleReporter;
use minitrace::collector::GlobalCollector;
use minitrace::prelude::*;

fn main() {
    minitrace::set_collector(GlobalCollector::new(ConsoleReporter, Config::default()));

    {
        let parent = SpanContext::random();
        let root = Span::root("root", parent);
        let _g = root.set_local_parent();
        let _g = LocalSpan::enter_with_local_parent("child");

        // do something ...
    }

    minitrace::flush();
}
