use minitrace::trace;

#[trace]
fn f(a: u64) {
    std::thread::sleep(std::time::Duration::from_millis(a));
}

fn main() {}
