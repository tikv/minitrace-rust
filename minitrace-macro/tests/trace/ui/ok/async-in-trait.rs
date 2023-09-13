#![cfg_attr(all(feature = "nightly", test), feature(async_fn_in_trait))]

#[cfg(all(feature = "nightly", test))]
mod tests {
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
}

fn main() {}
