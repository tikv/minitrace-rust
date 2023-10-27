use minitrace::trace;

#[trace(name = "Name", full_name = true)]
fn f() {}

fn main() {}
