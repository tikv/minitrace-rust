use minitrace::trace;

#[trace(name = "test_span")]
fn f(a: u32) -> u32 {
    a
}

fn main() {
    f(1);
}
