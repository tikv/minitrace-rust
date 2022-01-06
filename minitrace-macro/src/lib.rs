// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

// Intrumenting the async fn is not as straight forward as expected because `async_trait` rewrites `async fn`
// into a normal fn which returns `Box<impl Future>`, and this stops the macro from distinguishing `async fn` from `fn`.
// The following code reused the `async_trait` probes from [tokio-tracing](https://github.com/tokio-rs/tracing/blob/6a61897a5e834988ad9ac709e28c93c4dbf29116/tracing-attributes/src/expand.rs).

#![recursion_limit = "256"]

extern crate proc_macro;

#[macro_use]
extern crate proc_macro_error;

use proc_macro::TokenStream;
use quote::quote_spanned;
use syn::{
    spanned::Spanned, AttributeArgs, Block, Expr, ExprAsync, ExprCall, Generics, Item, ItemFn, Lit,
    Meta, MetaNameValue, NestedMeta, Path, Signature, Stmt,
};

#[proc_macro_attribute]
#[proc_macro_error]
pub fn trace(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as ItemFn);
    let args = Args::parse(syn::parse_macro_input!(args as AttributeArgs));

    // check for async_trait-like patterns in the block, and instrument
    // the future instead of the wrapper
    if let Some(internal_fun) = get_async_trait_info(&input.block, input.sig.asyncness.is_some()) {
        // let's rewrite some statements!
        let mut out_stmts = Vec::with_capacity(input.block.stmts.len());
        for stmt in &input.block.stmts {
            if stmt == internal_fun.source_stmt {
                match internal_fun.kind {
                    // async-trait <= 0.1.43
                    AsyncTraitKind::Function(fun) => {
                        out_stmts.push(gen_function(fun, args));
                    }
                    // async-trait >= 0.1.44
                    AsyncTraitKind::Async(async_expr) => {
                        // fallback if we couldn't find the '__async_trait' binding, might be
                        // useful for crates exhibiting the same behaviors as async-trait
                        let instrumented_block = gen_block(&async_expr.block, true, args);
                        let async_attrs = &async_expr.attrs;
                        out_stmts.push(quote! {
                            Box::pin(#(#async_attrs) * async move { #instrumented_block })
                        });
                    }
                }
                break;
            }
        }

        let vis = &input.vis;
        let sig = &input.sig;
        let attrs = &input.attrs;
        let func: proc_macro2::TokenStream = quote! {
            #(#attrs) *
            #vis #sig {
                #(#out_stmts) *
            }
        };
        func.into()
    } else {
        gen_function(&input, args).into()
    }
}

/// Given an existing function, generate an instrumented version of that function
fn gen_function(input: &ItemFn, args: Args) -> proc_macro2::TokenStream {
    let ItemFn {
        attrs,
        vis,
        block,
        sig,
    } = input;

    let Signature {
        output: return_type,
        inputs: params,
        unsafety,
        asyncness,
        constness,
        abi,
        ident,
        generics:
            Generics {
                params: gen_params,
                where_clause,
                ..
            },
        ..
    } = sig;

    let body = gen_block(block, asyncness.is_some(), args);

    quote::quote!(
        #(#attrs) *
        #vis #constness #unsafety #asyncness #abi fn #ident<#gen_params>(#params) #return_type
        #where_clause
        {
            #body
        }
    )
}

/// Instrument a block
fn gen_block(block: &Block, async_context: bool, args: Args) -> proc_macro2::TokenStream {
    let event = args.event;

    // Generate the instrumented function body.
    // If the function is an `async fn`, this will wrap it in an async block.
    // Otherwise, this will enter the span and then perform the rest of the body.
    if async_context {
        if args.enter_on_poll {
            quote_spanned!(block.span()=>
                minitrace::prelude::FutureExt::enter_on_poll(
                    async move { #block },
                    #event
                )
                .await
            )
        } else {
            quote_spanned!(block.span()=>
                minitrace::prelude::FutureExt::in_span(
                    async move { #block },
                    minitrace::prelude::Span::enter_with_local_parent( #event )
                )
                .await
            )
        }
    } else {
        if args.enter_on_poll {
            abort_call_site!("`enter_on_poll` can not be applied on non-async function");
        }

        quote_spanned!(block.span()=>
            let __guard = minitrace::prelude::LocalSpan::enter_with_local_parent( #event );
            #block
        )
    }
}

struct Args {
    event: String,
    enter_on_poll: bool,
}

impl Args {
    fn parse(input: AttributeArgs) -> Args {
        let name = match input.get(0) {
            Some(arg0) => match arg0 {
                NestedMeta::Lit(Lit::Str(name)) => name.value(),
                _ => abort!(arg0.span(), "expected string literal"),
            },
            None => abort_call_site!("expected at least one string literal"),
        };
        let enter_on_poll = match input.get(1) {
            Some(arg1) => match arg1 {
                NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                    path,
                    lit: Lit::Bool(b),
                    ..
                })) if path.is_ident("enter_on_poll") => b.value(),
                _ => abort!(arg1.span(), "expected `enter_on_poll = <bool>`"),
            },
            None => false,
        };
        if input.len() > 2 {
            abort_call_site!("too many arguments");
        }

        Args {
            event: name,
            enter_on_poll,
        }
    }
}

enum AsyncTraitKind<'a> {
    // old construction. Contains the function
    Function(&'a ItemFn),
    // new construction. Contains a reference to the async block
    Async(&'a ExprAsync),
}

struct AsyncTraitInfo<'a> {
    // statement that must be patched
    source_stmt: &'a Stmt,
    kind: AsyncTraitKind<'a>,
}

// Get the AST of the inner function we need to hook, if it was generated
// by async-trait.
// When we are given a function annotated by async-trait, that function
// is only a placeholder that returns a pinned future containing the
// user logic, and it is that pinned future that needs to be instrumented.
// Were we to instrument its parent, we would only collect information
// regarding the allocation of that future, and not its own span of execution.
// Depending on the version of async-trait, we inspect the block of the function
// to find if it matches the pattern
// `async fn foo<...>(...) {...}; Box::pin(foo<...>(...))` (<=0.1.43), or if
// it matches `Box::pin(async move { ... }) (>=0.1.44). We the return the
// statement that must be instrumented, along with some other informations.
// 'gen_body' will then be able to use that information to instrument the
// proper function/future.
// (this follows the approach suggested in
// https://github.com/dtolnay/async-trait/issues/45#issuecomment-571245673)
fn get_async_trait_info(block: &Block, block_is_async: bool) -> Option<AsyncTraitInfo<'_>> {
    // are we in an async context? If yes, this isn't a async_trait-like pattern
    if block_is_async {
        return None;
    }

    // list of async functions declared inside the block
    let inside_funs = block.stmts.iter().filter_map(|stmt| {
        if let Stmt::Item(Item::Fn(fun)) = &stmt {
            // If the function is async, this is a candidate
            if fun.sig.asyncness.is_some() {
                return Some((stmt, fun));
            }
        }
        None
    });

    // last expression of the block (it determines the return value
    // of the block, so that if we are working on a function whose
    // `trait` or `impl` declaration is annotated by async_trait,
    // this is quite likely the point where the future is pinned)
    let (last_expr_stmt, last_expr) = block.stmts.iter().rev().find_map(|stmt| {
        if let Stmt::Expr(expr) = stmt {
            Some((stmt, expr))
        } else {
            None
        }
    })?;

    // is the last expression a function call?
    let (outside_func, outside_args) = match last_expr {
        Expr::Call(ExprCall { func, args, .. }) => (func, args),
        _ => return None,
    };

    // is it a call to `Box::pin()`?
    let path = match outside_func.as_ref() {
        Expr::Path(path) => &path.path,
        _ => return None,
    };
    if !path_to_string(path).ends_with("Box::pin") {
        return None;
    }

    // Does the call take an argument? If it doesn't,
    // it's not gonna compile anyway, but that's no reason
    // to (try to) perform an out of bounds access
    if outside_args.is_empty() {
        return None;
    }

    // Is the argument to Box::pin an async block that
    // captures its arguments?
    if let Expr::Async(async_expr) = &outside_args[0] {
        // check that the move 'keyword' is present
        async_expr.capture?;

        return Some(AsyncTraitInfo {
            source_stmt: last_expr_stmt,
            kind: AsyncTraitKind::Async(async_expr),
        });
    }

    // Is the argument to Box::pin a function call itself?
    let func = match &outside_args[0] {
        Expr::Call(ExprCall { func, .. }) => func,
        _ => return None,
    };

    // "stringify" the path of the function called
    let func_name = match **func {
        Expr::Path(ref func_path) => path_to_string(&func_path.path),
        _ => return None,
    };

    // Was that function defined inside of the current block?
    // If so, retrieve the statement where it was declared and the function itself
    let (stmt_func_declaration, func) = inside_funs
        .into_iter()
        .find(|(_, fun)| fun.sig.ident == func_name)?;

    Some(AsyncTraitInfo {
        source_stmt: stmt_func_declaration,
        kind: AsyncTraitKind::Function(func),
    })
}

// Return a path as a String
fn path_to_string(path: &Path) -> String {
    use std::fmt::Write;
    // some heuristic to prevent too many allocations
    let mut res = String::with_capacity(path.segments.len() * 5);
    for i in 0..path.segments.len() {
        write!(&mut res, "{}", path.segments[i].ident)
            .expect("writing to a String should never fail");
        if i < path.segments.len() - 1 {
            res.push_str("::");
        }
    }
    res
}
