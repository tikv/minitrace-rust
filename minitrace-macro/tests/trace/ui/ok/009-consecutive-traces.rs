use minitrace::trace;

// Revisit this in relation to issue #134
#[trace]
#[allow(unused_braces)]
#[trace]
#[warn(unused_braces)]
fn f() {}

fn main() {}
