use minitrace::trace;

#[trace(properties = { a: "b" })]
fn f() {}

fn main() {}
