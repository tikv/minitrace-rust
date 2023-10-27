use minitrace::trace;

#[trace(name = "Name", full_path = true)]
fn f() {}

fn main() {}
