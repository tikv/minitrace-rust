use minitrace::trace;

#[trace(name = "a", enter_on_poll)]
fn f() {}

fn main() {
    f();
}
