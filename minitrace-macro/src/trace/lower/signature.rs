use crate::trace::lower::lifetime::*;

use syn::visit_mut::VisitMut;

pub fn transform_sig(sig: &mut syn::Signature, has_self: bool, is_local: bool) {
    sig.fn_token.span = sig.asyncness.take().unwrap().span;

    let ret = match &sig.output {
        syn::ReturnType::Default => quote::quote!(()),
        syn::ReturnType::Type(_, ret) => quote::quote!(#ret),
    };

    let default_span = sig
        .ident
        .span()
        .join(sig.paren_token.span)
        .unwrap_or_else(|| sig.ident.span());

    let mut lifetimes = CollectLifetimes::new("'life", default_span);
    for arg in sig.inputs.iter_mut() {
        match arg {
            syn::FnArg::Receiver(arg) => lifetimes.visit_receiver_mut(arg),
            syn::FnArg::Typed(arg) => lifetimes.visit_type_mut(&mut arg.ty),
        }
    }

    for param in sig.generics.params.iter() {
        match param {
            syn::GenericParam::Type(param) => {
                let param = &param.ident;
                let span = param.span();
                where_clause_or_default(&mut sig.generics.where_clause)
                    .predicates
                    .push(syn::parse_quote_spanned!(span=> #param: 'minitrace));
            }
            syn::GenericParam::Lifetime(param) => {
                let param = &param.lifetime;
                let span = param.span();
                where_clause_or_default(&mut sig.generics.where_clause)
                    .predicates
                    .push(syn::parse_quote_spanned!(span=> #param: 'minitrace));
            }
            syn::GenericParam::Const(_) => {}
        }
    }

    if sig.generics.lt_token.is_none() {
        sig.generics.lt_token = Some(syn::Token![<](sig.ident.span()));
    }
    if sig.generics.gt_token.is_none() {
        sig.generics.gt_token = Some(syn::Token![>](sig.paren_token.span));
    }

    for (idx, elided) in lifetimes.elided.iter().enumerate() {
        sig.generics.params.insert(idx, syn::parse_quote!(#elided));
        where_clause_or_default(&mut sig.generics.where_clause)
            .predicates
            .push(syn::parse_quote_spanned!(elided.span()=> #elided: 'minitrace));
    }

    sig.generics
        .params
        .insert(0, syn::parse_quote_spanned!(default_span=> 'minitrace));

    if has_self {
        let bound_span = sig.ident.span();
        let bound = match sig.inputs.iter().next() {
            Some(syn::FnArg::Receiver(syn::Receiver {
                reference: Some(_),
                mutability: None,
                ..
            })) => syn::Ident::new("Sync", bound_span),
            Some(syn::FnArg::Typed(arg))
                if match (arg.pat.as_ref(), arg.ty.as_ref()) {
                    (syn::Pat::Ident(pat), syn::Type::Reference(ty)) => {
                        pat.ident == "self" && ty.mutability.is_none()
                    }
                    _ => false,
                } =>
            {
                syn::Ident::new("Sync", bound_span)
            }
            _ => syn::Ident::new("Send", bound_span),
        };

        let where_clause = where_clause_or_default(&mut sig.generics.where_clause);
        where_clause.predicates.push(if is_local {
            syn::parse_quote_spanned!(bound_span=> Self: 'minitrace)
        } else {
            syn::parse_quote_spanned!(bound_span=> Self: ::core::marker::#bound + 'minitrace)
        });
    }

    for (i, arg) in sig.inputs.iter_mut().enumerate() {
        match arg {
            syn::FnArg::Receiver(syn::Receiver {
                reference: Some(_), ..
            }) => {}
            syn::FnArg::Receiver(arg) => arg.mutability = None,
            syn::FnArg::Typed(arg) => {
                if let syn::Pat::Ident(ident) = &mut *arg.pat {
                    ident.by_ref = None;
                    //ident.mutability = None;
                } else {
                    let positional = positional_arg(i, &arg.pat);
                    let m = mut_pat(&mut arg.pat);
                    arg.pat = syn::parse_quote!(#m #positional);
                }
            }
        }
    }

    let ret_span = sig.ident.span();
    let bounds = if is_local {
        quote::quote_spanned!(ret_span=> 'minitrace)
    } else {
        quote::quote_spanned!(ret_span=> ::core::marker::Send + 'minitrace)
    };
    sig.output = syn::parse_quote_spanned! {ret_span=>
        -> impl ::core::future::Future<Output = #ret> + #bounds
    };
}

fn positional_arg(i: usize, pat: &syn::Pat) -> syn::Ident {
    quote::format_ident!("__arg{}", i, span = syn::spanned::Spanned::span(&pat))
}

fn mut_pat(pat: &mut syn::Pat) -> Option<syn::Token![mut]> {
    let mut visitor = HasMutPat(None);
    visitor.visit_pat_mut(pat);
    visitor.0
}

fn has_self_in_token_stream(tokens: proc_macro2::TokenStream) -> bool {
    tokens.into_iter().any(|tt| match tt {
        proc_macro2::TokenTree::Ident(ident) => ident == "Self",
        proc_macro2::TokenTree::Group(group) => has_self_in_token_stream(group.stream()),
        _ => false,
    })
}

fn where_clause_or_default(clause: &mut Option<syn::WhereClause>) -> &mut syn::WhereClause {
    clause.get_or_insert_with(|| syn::WhereClause {
        where_token: Default::default(),
        predicates: syn::punctuated::Punctuated::new(),
    })
}

struct HasMutPat(Option<syn::Token![mut]>);

impl syn::visit_mut::VisitMut for HasMutPat {
    fn visit_pat_ident_mut(&mut self, i: &mut syn::PatIdent) {
        if let Some(m) = i.mutability {
            self.0 = Some(m);
        } else {
            syn::visit_mut::visit_pat_ident_mut(self, i);
        }
    }
}

pub struct HasSelf(pub bool);

impl syn::visit_mut::VisitMut for HasSelf {
    fn visit_expr_path_mut(&mut self, expr: &mut syn::ExprPath) {
        self.0 |= expr.path.segments[0].ident == "Self";
        syn::visit_mut::visit_expr_path_mut(self, expr);
    }

    fn visit_pat_path_mut(&mut self, pat: &mut syn::PatPath) {
        self.0 |= pat.path.segments[0].ident == "Self";
        syn::visit_mut::visit_pat_path_mut(self, pat);
    }

    fn visit_type_path_mut(&mut self, ty: &mut syn::TypePath) {
        self.0 |= ty.path.segments[0].ident == "Self";
        syn::visit_mut::visit_type_path_mut(self, ty);
    }

    fn visit_receiver_mut(&mut self, _arg: &mut syn::Receiver) {
        self.0 = true;
    }

    fn visit_item_mut(&mut self, _: &mut syn::Item) {
        // Do not recurse into nested items.
    }

    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        if !contains_fn(mac.tokens.clone()) {
            self.0 |= has_self_in_token_stream(mac.tokens.clone());
        }
    }
}

fn contains_fn(tokens: proc_macro2::TokenStream) -> bool {
    tokens.into_iter().any(|tt| match tt {
        proc_macro2::TokenTree::Ident(ident) => ident == "fn",
        proc_macro2::TokenTree::Group(group) => contains_fn(group.stream()),
        _ => false,
    })
}
