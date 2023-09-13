use minitrace::trace;

#[trace]
async fn test_async() {}

#[trace]
fn test_sync() {}

#[cfg_attr(feature = "tk", tokio::main)]
#[cfg_attr(not(feature = "tk"), async_std::main)]
async fn main() {
    let (root, collector) = minitrace::Span::root("root");
    {
        let _g = root.set_local_parent();
        test_async(1).await;
        test_sync();
    }
    drop(root);
    let records: Vec<minitrace::collector::SpanRecord> = futures::executor::block_on(collector.collect());

    // Enforce future send
    // tokio::Runtime::spawn(test_async().await);
    test_async().await;

    test_sync();
}