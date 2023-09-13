use minitrace::prelude::*;
use minitrace::trace;

#[derive(Debug)]
struct test;

#[minitrace::trace()]
fn f(a: usize) -> usize {
    a * 2
}

fn main() {
    f(2);
}
