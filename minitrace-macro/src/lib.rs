//! A procedural macro attribute for instrumenting functions with [`minitrace`].
//!
//! [`minitrace`] is a performance-focused library for instrumenting Rust programs to collect
//! structured, event-based diagnostic information. This crate provides the
//! [`#[trace]`][trace] procedural macro attribute.
//!
//! Note that this macro is also re-exported by the main `minitrace` crate.
//!
//! *Compiler support: [requires `rustc` 1.49+][msrv]*
//!
//! [msrv]: #supported-rust-versions
//!
//! ## Getting Started
//!
//! This crate is included as part of the Minitrace crate.  In general, you do
//! ***not** need to add this crate to your project's `Cargo.toml`.
//! However, you may wish to have this crate as a dependency only in
//! development, for this use case, please see [Development Only] in the
//! [Usage] section.
//!
//! ## Usage
//!
//! ### Development Only
//!
//! To have this crate as a dependency only in development:
//!
//! ```toml
//! [package]
//! ...
//! resolver = "2"
//!
//! [dependencies]
//! minitrace = {version = "0.5", default-features = false}
//!
//! [build-dependencies]
//! minitrace = {version = "0.5", features = ["attributes", "enable"]}
//! ```
//!
//! ## Examples
//!
//! Please review the contents of the `examples` folder, as well as the integration test suite under `tests`.
//!

// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! An attribute macro designed to eliminate boilerplate code for [`minitrace`](https://crates.io/crates/minitrace).

// Instrumenting the async fn is not as straight forward as expected because `async_trait` rewrites `async fn`
// into a normal fn which returns `Box<impl Future>`, and this stops the macro from distinguishing `async fn` from `fn`.
// The following code reused the `async_trait` probes from [tokio-tracing](https://github.com/tokio-rs/tracing/blob/6a61897a5e834988ad9ac709e28c93c4dbf29116/tracing-attributes/src/expand.rs).

#![recursion_limit = "256"]

mod trace;

extern crate proc_macro;

/// An attribute macro designed to eliminate boilerplate code.
///
/// This macro automatically creates a span for the annotated function. The span name defaults to the function
/// name but can be customized by passing a string literal as an argument using the `name` parameter.
///
/// The `#[trace]` attribute requires a local parent context to function correctly. Ensure that
/// the function annotated with `#[trace]` is called within the scope of `Span::set_local_parent()`.
///
/// # Examples
///
/// ```
/// use minitrace::prelude::*;
///
/// #[trace]
/// fn foo() {
///     // ...
/// }
///
/// #[trace]
/// async fn bar() {
///     // ...
/// }
///
/// #[trace(name = "qux", enter_on_poll = true)]
/// async fn baz() {
///     // ...
/// }
/// ```
///
/// The code snippets above are equivalent to:
///
/// ```
/// # use minitrace::prelude::*;
/// # use minitrace::local::LocalSpan;
/// fn foo() {
///     let __guard__ = LocalSpan::enter_with_local_parent("foo");
///     // ...
/// }
///
/// async fn bar() {
///     async {
///         // ...
///     }
///     .in_span(Span::enter_with_local_parent("bar"))
///     .await
/// }
///
/// async fn baz() {
///     async {
///         // ...
///     }
///     .enter_on_poll("qux")
///     .await
/// }
/// ```
#[proc_macro_attribute]
pub fn trace(
    args: proc_macro::TokenStream,
    items: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let trace = syn::parse2::<trace::Trace>(args.into());
    let input: trace::Trace = match trace {
        Ok(trace) => trace,
        Err(e) => {
            return token_stream_with_error(items.into(), e).into();
        }
    };

    let models = trace::analyze(input, items.into());

    let quotes = trace::lower(models);

    let rust = trace::generate(quotes);

    rust.into()
}

// If any of the steps for this macro fail, we still want to expand to an item
// that is as close to the expected output as possible.
// This helps out IDEs such that completions and other related features keep
// working.
fn token_stream_with_error(
    mut tokens: proc_macro2::TokenStream,
    error: syn::Error,
) -> proc_macro2::TokenStream {
    tokens.extend(error.into_compile_error());
    tokens
}
