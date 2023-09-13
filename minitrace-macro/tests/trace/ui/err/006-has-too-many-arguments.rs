use minitrace::trace;

#[trace(a=true, b=true, c=true, d=true)]
fn f() {}

fn main() {}
