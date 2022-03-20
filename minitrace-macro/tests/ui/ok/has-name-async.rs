use minitrace::trace;

// This Tracing crate like-syntax
#[allow(unused_braces)]
#[trace["test-span"]]
async fn f(a: u32) -> u32 { a }

#[tokio::main]
async fn main() {
    f(1).await;
}
