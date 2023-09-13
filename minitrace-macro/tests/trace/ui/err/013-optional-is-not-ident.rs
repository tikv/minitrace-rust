use minitrace::trace;

#[trace(name = "a", some::unknown=true)]
fn f() {}

fn main() {}
