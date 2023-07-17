// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use super::global_collector::Reporter;
use super::SpanRecord;

pub struct TerminalReporter;

impl Reporter for TerminalReporter {
    fn report(&mut self, spans: &[SpanRecord]) -> Result<(), Box<dyn std::error::Error>> {
        for span in spans {
            eprintln!("{:#?}", span);
        }
        Ok(())
    }
}
