use minitrace::trace;

#[allow(unused_braces)]
#[trace(name = struct)]
#[warn(unused_braces)]
fn f() {}

fn main() {}
