// Parse TokenStream
//
// Parse attribute arguments, which arrive as a `proc_macro::TokenStream`,
// into a `Vector` of `syn::NestedMeta` items.
//
// The input stream comes from the `trace::validate::validate` function.
// The output vector goes to the `trace::analyze::analyze` function.

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scope {
    Local,
    Threads,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    pub default: syn::LitBool,
    pub name: syn::LitStr,
    pub validated: syn::LitBool,
    pub enter_on_poll: syn::LitBool,

    pub scope: Option<Scope>, // Scope::Local, Scope::Thread, etc.
    pub parent: Option<syn::LitStr>,
    pub recorder: Option<syn::Ident>,
    pub recurse: Option<syn::LitBool>,
    pub root: Option<syn::LitBool>,
    pub variables: Option<syn::ExprArray>,
    pub async_trait: Option<syn::LitBool>,
    pub async_fn: Option<syn::LitBool>,
}

impl syn::parse::Parse for Trace {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut enter_on_poll = None;
        let mut name = None;
        let mut name_set = false;

        let mut parsed =
            syn::punctuated::Punctuated::<syn::MetaNameValue, syn::Token![,]>::parse_terminated(
                input,
            )?;
        let arg_n = parsed.len();
        if arg_n > 3 {
            // tests/trace/ui/err/has-too-many-arguments.rs
            //abort_call_site!(ERROR; help = HELP)
            let e = syn::Error::new(
                syn::spanned::Spanned::span(&parsed),
                "Too many arguments. This attribute takes up to two (2) arguments",
            );
            return Err(e);
        }
        for kv in parsed.clone() {
            if kv.path.is_ident("enter_on_poll") {
                if enter_on_poll.is_some() {
                    let e = syn::Error::new(
                        syn::spanned::Spanned::span(&kv),
                        "`enter_on_poll` provided twice",
                    );
                    return Err(e);
                } else if let syn::Lit::Bool(v) = kv.lit {
                    enter_on_poll = Some(v);
                } else {
                    let e = syn::Error::new(
                        syn::spanned::Spanned::span(&kv),
                        "`enter_on_poll` value should be an boolean",
                    );
                    return Err(e);
                }
            } else if kv.path.is_ident("name") {
                name_set = true;
                if name.is_some() {
                    let e =
                        syn::Error::new(syn::spanned::Spanned::span(&kv), "`name` provided twice");
                    return Err(e);
                } else if let syn::Lit::Str(v) = kv.lit {
                    name = Some(v);
                } else {
                    let e = syn::Error::new(
                        syn::spanned::Spanned::span(&kv),
                        "`name` value should be a string",
                    );
                    return Err(e);
                }
            } else {
                let e = syn::Error::new(syn::spanned::Spanned::span(&kv), "unknown option");
                return Err(e);
            }
        }

        if !name_set {
            let name_pair: syn::MetaNameValue = syn::parse_quote!(name = "__default");
            parsed.push(name_pair);
            name = Some(syn::LitStr::new(
                "__default",
                proc_macro2::Span::call_site(),
            ));
        }
        // Validate supported combinations
        match (enter_on_poll, name) {
            (Some(enter_on_poll), Some(name)) => {
                let default = syn::LitBool::new(false, proc_macro2::Span::call_site());
                let validated = syn::LitBool::new(true, proc_macro2::Span::call_site());
                Ok(Self {
                    default,
                    enter_on_poll,
                    name,
                    validated,
                    ..Default::default()
                })
            }
            (None, None) => Err(syn::Error::new(
                syn::spanned::Spanned::span(&parsed),
                "missing both `enter_on_poll` and `name`",
            )),
            (None, Some(name)) => {
                let default = syn::LitBool::new(false, proc_macro2::Span::call_site());
                let validated = syn::LitBool::new(true, proc_macro2::Span::call_site());
                Ok(Self {
                    default,
                    name,
                    validated,
                    ..Default::default()
                })
            }
            (Some(enter_on_poll), None) => {
                let default = syn::LitBool::new(false, proc_macro2::Span::call_site());
                let validated = syn::LitBool::new(true, proc_macro2::Span::call_site());
                let name = syn::LitStr::new("__default", proc_macro2::Span::call_site());
                Ok(Self {
                    default,
                    enter_on_poll,
                    name,
                    validated,
                    ..Default::default()
                })
            }
        }
    }
}

impl Default for Trace {
    fn default() -> Self {
        // Indicate when these defaults have changed
        let default = syn::LitBool::new(true, proc_macro2::Span::call_site());
        // Indicate when these values have been validated
        let validated = syn::LitBool::new(false, proc_macro2::Span::call_site());
        let name = syn::LitStr::new("__default", proc_macro2::Span::call_site());
        let scope = Some(Scope::Local);
        let enter_on_poll = syn::LitBool::new(false, proc_macro2::Span::call_site());
        let recorder = Some(proc_macro2::Ident::new(
            "span",
            proc_macro2::Span::call_site(),
        ));
        let recurse = Some(syn::LitBool::new(false, proc_macro2::Span::call_site()));
        let root = Some(syn::LitBool::new(false, proc_macro2::Span::call_site()));
        let variables = Some(syn::parse_quote!([]));
        let parent = Some(syn::LitStr::new(
            "__default",
            proc_macro2::Span::call_site(),
        ));
        let async_trait = Some(syn::LitBool::new(false, proc_macro2::Span::call_site()));
        let async_fn = Some(syn::LitBool::new(false, proc_macro2::Span::call_site()));

        Self {
            name,
            async_trait,
            async_fn,
            default,
            enter_on_poll,
            parent,
            recorder,
            recurse,
            root,
            scope,
            variables,
            validated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utilities::*;

    #[test]
    fn valid_trace_001() {
        // let ts = syn::parse::Parser::parse_str(syn::Attribute::parse_outer, "#[trace]").unwrap();
        // let args: proc_macro2::TokenStream = ts
        //     .iter()
        //     .map(|attr| attr.parse_args::<syn::NestedMeta>().unwrap())
        //     .collect();
        let args = quote::quote!(name = "a", enter_on_poll = false,);
        let actual = syn::parse2::<Trace>(args).unwrap();
        let expected = Trace {
            default: syn::LitBool::new(false, proc_macro2::Span::call_site()),
            enter_on_poll: syn::LitBool::new(false, proc_macro2::Span::call_site()),
            name: syn::LitStr::new("a", proc_macro2::Span::call_site()),
            validated: syn::LitBool::new(true, proc_macro2::Span::call_site()),
            ..Default::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn valid_trace_002() {
        let args = quote::quote!(name = "a", enter_on_poll = false,);
        let actual = syn::parse2::<Trace>(args).unwrap();
        let expected = Trace {
            default: syn::LitBool::new(false, proc_macro2::Span::call_site()),
            enter_on_poll: syn::LitBool::new(false, proc_macro2::Span::call_site()),
            name: syn::LitStr::new("a", proc_macro2::Span::call_site()),
            validated: syn::LitBool::new(true, proc_macro2::Span::call_site()),
            ..Default::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn valid_trace_003() {
        let args = quote::quote!(enter_on_poll = false,);
        let actual = syn::parse2::<Trace>(args).unwrap();
        let expected = Trace {
            default: syn::LitBool::new(false, proc_macro2::Span::call_site()),
            enter_on_poll: syn::LitBool::new(false, proc_macro2::Span::call_site()),
            name: syn::LitStr::new("__default", proc_macro2::Span::call_site()),
            validated: syn::LitBool::new(true, proc_macro2::Span::call_site()),
            ..Default::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn valid_trace_004() {
        let args = quote::quote!(name = "a",);
        let actual = syn::parse2::<Trace>(args).unwrap();
        let expected = Trace {
            default: syn::LitBool::new(false, proc_macro2::Span::call_site()),
            name: syn::LitStr::new("a", proc_macro2::Span::call_site()),
            validated: syn::LitBool::new(true, proc_macro2::Span::call_site()),
            ..Default::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn invalid_trace_001() {
        let args = quote::quote!(name = "a", name = "b", enter_on_poll = false,);
        let actual = match syn::parse2::<Trace>(args.clone()) {
            Err(error) => error,
            _ => syn::Error::new(syn::spanned::Spanned::span(""), "error"),
        };
        let expected: syn::Error =
            syn::Error::new(syn::spanned::Spanned::span(&args), "`name` provided twice");
        assert_eq_text!(&format!("{:#?}", expected), &format!("{:#?}", actual));
    }

    #[test]
    fn invalid_trace_002() {
        let args = quote::quote!(name = "a", enter_on_poll = true, enter_on_poll = false,);
        let actual = match syn::parse2::<Trace>(args.clone()) {
            Err(error) => error,
            _ => syn::Error::new(syn::spanned::Spanned::span(""), "error"),
        };
        let expected: syn::Error = syn::Error::new(
            syn::spanned::Spanned::span(&args),
            "`enter_on_poll` provided twice",
        );
        assert_eq_text!(&format!("{:#?}", expected), &format!("{:#?}", actual));
    }
}
