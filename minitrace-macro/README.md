# minitrace-macro

[![Documentation](https://docs.rs/minitrace-macro/badge.svg)](https://docs.rs/minitrace-macro/)
[![Crates.io](https://img.shields.io/crates/v/minitrace-macro.svg)](https://crates.io/crates/minitrace-macro)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

Provides an attribute-macro `trace` to help get rid of boilerplate.

## Usage

### Dependency

```toml
[dependencies]
minitrace = "0.4" # minitrace-macro is within minitrace::prelude
```

### Synchronous Function

```rust
use minitrace::prelude::*;

#[trace("foo")]
fn foo() {
    // function body
}

// ** WILL BE TRANSLATED INTO: **
//
// fn foo() {
//     let __guard = LocalSpan::enter_with_local_parent("foo");
//     {
//         // function body
//     }
// }
```

### Asynchronous Function

```rust
use minitrace::prelude::*;

#[trace("bar")]
async fn bar() {
    // function body
}

// ** WILL BE TRANSLATED INTO: **
//
// fn bar() -> impl core::future::Future<Output = ()> {
//     async {
//         // function body
//     }
//     .in_span(Span::enter_with_local_parent("bar"))
// }


#[trace("qux", enter_on_poll = true)]
async fn qux() {
    // function body
}

// ** WILL BE TRANSLATED INTO: **
//
// fn qux() -> impl core::future::Future<Output = ()> {
//     async {
//         // function body
//     }
//     .enter_on_poll("qux")
// }
```

### ⚠️ Local Parent Needed 

A function instrumented by `trace` always require a local parent in the context. Make sure that the caller is within the scope of `Span::set_local_parent()`.

```rust
use minitrace::prelude::*;

#[trace("foo")]
fn foo() {}

#[trace("bar")]
async fn bar() {}

let (root, collector) = Span::root("root");

{
    foo(); // This `foo` will __not__ be traced.
}

{
    let _g = root.set_local_parent();
    foo(); // This `foo` will be traced.
}

{
    runtime::spawn(bar()); // This `bar` will __not__ be traced.
}

{
    let _g = root.set_local_parent();
    runtime::spawn(bar()); // This `bar` will be traced.
}
```
