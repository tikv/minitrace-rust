#![feature(async_fn_in_trait)]

trait MyTrait {
    async fn work(&self) -> usize;
}

struct MyStruct;

impl MyTrait for MyStruct {
    // #[logcall::logcall("info")]
    #[minitrace::trace]
    async fn work(&self) -> usize {
        todo!()
    }
}

fn main() {}
