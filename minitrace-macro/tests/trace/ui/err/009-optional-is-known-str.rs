use minitrace::trace;

#[trace(name = "a", "enter_on_poll" = true)]
fn f() {}

fn main() {
    f();
}
