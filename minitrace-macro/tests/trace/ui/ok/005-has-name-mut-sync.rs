use minitrace::trace;

#[trace(name = "test_span")]
fn f(mut a: u32) -> u32 {
    a = a + 1;
    a
}

fn main() {
    f(1);
}
