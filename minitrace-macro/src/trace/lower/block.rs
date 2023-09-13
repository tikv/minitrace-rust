use crate::trace::lower::TracedItem;

use syn::spanned::Spanned;

/// Instrument a block
pub fn gen_block(
    block: &syn::Block,
    async_context: bool,
    traced_item: TracedItem,
) -> proc_macro2::TokenStream {
    let event = traced_item.name.value();

    // Generate the instrumented function body.
    // If the function is an `async fn`, this will wrap it in an async block.
    // Otherwise, this will enter the span and then perform the rest of the body.
    if async_context {
        if traced_item.enter_on_poll.value {
            quote::quote_spanned!(block.span()=>
                minitrace::future::FutureExt::enter_on_poll(
                    async move { #block },
                    #event
                )
            )
        } else {
            quote::quote_spanned!(block.span()=>
                minitrace::future::FutureExt::in_span(
                    async move { #block },
                    minitrace::Span::enter_with_local_parent( #event )
                )
            )
        }
    } else {
        if traced_item.enter_on_poll.value {
            let e = syn::Error::new(
                syn::spanned::Spanned::span(&async_context),
                "`enter_on_poll` can not be applied on non-async function",
            );
            let tokens = quote::quote_spanned!(block.span()=>
                let __guard = minitrace::local::LocalSpan::enter_with_local_parent( #event );
                #block
            );
            return crate::token_stream_with_error(tokens, e);
        }

        quote::quote_spanned!(block.span()=>
            let __guard = minitrace::local::LocalSpan::enter_with_local_parent( #event );
            #block
        )
    }
}
