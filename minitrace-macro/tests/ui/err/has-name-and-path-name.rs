use minitrace::trace;

#[trace(name = "Name", path_name = true)]
fn f() {}

fn main() {}
