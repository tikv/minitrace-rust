use std::time::Duration;

use minitrace::collector::Config;
use minitrace::collector::ConsoleReporter;
use minitrace::prelude::*;

fn main() {
    minitrace::set_reporter(ConsoleReporter, Config::default());

    {
        let parent = SpanContext::random();
        let mut root = Span::root("light work", parent);
        let _span_guard = root.set_local_parent();

        expensive_work(Duration::from_millis(50));

        // Cancel the trace to avoid reporting if it's too short.
        if root.elapsed() < Some(Duration::from_millis(100)) {
            root.cancel();
        }
    };

    {
        let parent = SpanContext::random();
        let mut root = Span::root("heavy work", parent);
        let _span_guard = root.set_local_parent();

        expensive_work(Duration::from_millis(200));

        // This trace will be reported.
        if root.elapsed() < Some(Duration::from_millis(100)) {
            root.cancel();
        }
    };

    minitrace::flush();
}

#[trace]
fn expensive_work(time: Duration) {
    std::thread::sleep(time);
}
