use minitrace::trace;

#[trace(name = "a", some_unknown = true)]
fn f() {}

fn main() {
    f();
}
