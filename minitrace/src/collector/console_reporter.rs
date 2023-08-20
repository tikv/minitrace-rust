// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use super::global_collector::Reporter;
use super::SpanRecord;

/// A console reporter that prints span records to the stderr.
pub struct ConsoleReporter;

impl Reporter for ConsoleReporter {
    fn report(&mut self, spans: &[SpanRecord]) {
        for span in spans {
            eprintln!("{:#?}", span);
        }
    }
}
