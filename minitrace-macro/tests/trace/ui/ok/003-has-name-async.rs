use minitrace::trace;

#[trace(name = "test_span")]
async fn f(a: u32) -> u32 {
    a
}

#[tokio::main]
async fn main() {
    f(1).await;
}
