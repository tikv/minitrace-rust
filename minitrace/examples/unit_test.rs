#![allow(dead_code)]
#![allow(unused_imports)]

use minitrace::prelude::*;
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
    where F: FnOnce() -> Result<()> + 'static {
        minitrace::set_reporter(ConsoleReporter, Config::default());
        {
            let root = Span::root(closure_name::<F>(), SpanContext::random());
            let _guard = root.set_local_parent();
            test().expect("test success");
        }
        minitrace::flush();
    }

    pub fn setup_minitrace_async<F, Fut>(test: F)
    where
        F: FnOnce() -> Fut + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        minitrace::set_reporter(ConsoleReporter, Config::default());
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(3)
            .enable_all()
            .build()
            .unwrap();
        let root = Span::root(closure_name::<F>(), SpanContext::random());
        rt.block_on(test().in_span(root)).unwrap();
        minitrace::flush();
    }

    pub fn closure_name<F: std::any::Any>() -> &'static str {
        let full_name = std::any::type_name::<F>();
        full_name
            .rsplit("::")
            .find(|name| *name != "{{closure}}")
            .unwrap()
    }
}

fn main() {}
