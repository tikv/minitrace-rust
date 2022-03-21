use minitrace::trace;

#[trace("test-span")]
async fn f(mut a: u32) -> u32 {
    a
}

#[tokio::main]
async fn main() {
    f(1).await;
}
