#![allow(dead_code)]
#![allow(unused_imports)]

use test_harness::test;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test(harness = test_util::setup_minitrace)]
#[trace]
fn test_sync() -> Result<()> {
    std::thread::sleep(std::time::Duration::from_millis(50));
    Ok(())
}

#[test(harness = test_util::setup_minitrace_async)]
#[trace]
async fn test_async() -> Result<()> {
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(())
}

#[cfg(test)]
mod test_util {
    use minitrace::collector::Config;
    use minitrace::collector::ConsoleReporter;
    use minitrace::prelude::*;

    use super::*;

    pub fn setup_minitrace<F>(test: F)
    where F: FnOnce() -> Result<()> {
        minitrace::set_reporter(ConsoleReporter, Config::default());
        {
            let root = Span::root(
                "unit test",
                SpanContext::new(TraceId::random(), SpanId::default()),
            );
            let _guard = root.set_local_parent();
            test().expect("test success");
        }
        minitrace::flush();
    }

    pub fn setup_minitrace_async<F, Fut>(test: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        minitrace::set_reporter(ConsoleReporter, Config::default());
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(3)
            .enable_all()
            .build()
            .unwrap();
        let root = Span::root(
            "unit test",
            SpanContext::new(TraceId::random(), SpanId::default()),
        );
        rt.block_on(test().in_span(root)).unwrap();
        minitrace::flush();
    }
}

fn main() {}
