use minitrace::trace;

#[trace(properties = { "a": "{{b}" })]
fn f(b: u8) {}

fn main() {}
