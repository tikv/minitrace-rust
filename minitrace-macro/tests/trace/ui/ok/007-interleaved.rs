use minitrace::trace;

#[allow(unused_braces)]
#[trace]
#[warn(unused_braces)]
fn f(a: u64) {
    std::thread::sleep(std::time::Duration::from_millis(a));
}

fn main() {}
