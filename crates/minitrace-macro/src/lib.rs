// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![feature(proc_macro_diagnostic)]
#![recursion_limit = "256"]

extern crate proc_macro;

#[macro_use]
extern crate proc_macro_error;

use proc_macro::TokenStream;
use syn::spanned::Spanned;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn trace(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let event = syn::parse_macro_input!(args as syn::Expr);

    let syn::ItemFn {
        attrs,
        vis,
        block,
        sig,
    } = input;

    let syn::Signature {
        output: return_type,
        inputs: params,
        unsafety,
        asyncness,
        constness,
        abi,
        ident,
        generics:
            syn::Generics {
                params: gen_params,
                where_clause,
                ..
            },
        ..
    } = sig;

    if asyncness.is_some() {
        abort!(
            asyncness,
            "Unexpected async\nIf want to trace async function, consider `minitrace::trace_async`"
        );
    };

    quote::quote!(
        #(#attrs) *
        #vis #constness #unsafety #asyncness #abi fn #ident<#gen_params>(#params) #return_type
        #where_clause
        {
            let _guard = new_span(#event);
            #block
        }
    )
    .into()
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn trace_async(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let event = syn::parse_macro_input!(args as syn::Expr);

    let syn::ItemFn {
        attrs,
        vis,
        block,
        sig,
    } = input;

    let syn::Signature {
        output: return_type,
        inputs: params,
        unsafety,
        asyncness,
        constness,
        abi,
        ident,
        generics:
            syn::Generics {
                params: gen_params,
                where_clause,
                ..
            },
        ..
    } = sig;

    let body = if asyncness.is_some() {
        let async_kwd = syn::token::Async { span: block.span() };
        let await_kwd = syn::Ident::new("await", block.span());
        quote::quote_spanned! {block.span() =>
            #async_kwd move { #block }
                .in_new_span(#event)
                .#await_kwd
        }
    } else {
        // hack for `async_trait`
        // See https://docs.rs/async-trait/0.1.31/async_trait/
        quote::quote_spanned! {block.span() =>
            std::boxed::Box::pin(#block.in_new_span(#event))
        }
    };

    quote::quote!(
        #(#attrs) *
        #vis #constness #unsafety #asyncness #abi fn #ident<#gen_params>(#params) #return_type
        #where_clause
        {
            #body
        }
    )
    .into()
}
