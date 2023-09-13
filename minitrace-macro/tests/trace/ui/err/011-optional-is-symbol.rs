use minitrace::trace;

#[trace(name = "a", type)]
fn f() {}

fn main() {
    f();
}
