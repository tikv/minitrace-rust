use minitrace::trace;

#[trace(name = "test_span", enter_on_poll = true, name = "not_this")]
fn f(a: u32) -> u32 {
    a
}

fn main() {
    f(1);
}
