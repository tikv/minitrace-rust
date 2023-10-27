use minitrace::trace;

#[trace(enter_on_poll = true)]
fn f() {}

fn main() {}
