/// Implementation Notes:
///
/// Check for `async-trait`-like patterns in the block, and instrument the
/// future instead of the wrapper.
///
/// Instrumenting the `async fn` is not as straight forward as expected because
/// `async_trait` rewrites `async fn` into a normal `fn` which returns
/// `Pin<Box<dyn Future + Send + 'async_trait>>`, and this stops the macro from
/// distinguishing `async fn` from `fn`.
///
/// The following logic and code is from the `async-trait` probes from
/// [tokio-tracing][tokio-logic].
/// The Tokio logic is required for detecting the `async fn` that is already
/// transformed to `fn -> Pin<Box<dyn Future + Send + 'async_trait>>` by
/// `async-trait`.
/// We have to distinguish this case from `fn -> impl Future` that is written
/// by the user because for the latter, we instrument it like normal `fn`
/// instead of `async fn`.
///
/// The reason why we elaborate `async fn` into `fn -> impl Future`:
/// For an `async fn foo()`, we have to instrument the
/// `Span::enter_with_local_parent()` in the first call to `foo()`, but not in
/// the `poll()` or `.await`, because it's the only chance that
/// local parent is present in that context.
///
/// [tokio-logic]: https://github.com/tokio-rs/tracing/blob/6a61897a5e834988ad9ac709e28c93c4dbf29116/tracing-attributes/src/expand.rs

// Trace Attribute Features
//
// The feature set for the `trace` attribute is evolving, heading to a 1.0
// release.  The following features are under discussion.  Implementation
// will be non-trivial until issues #136 and issue #137 are resolved.
// A consequence of this is that implementation will need to be incremental
// rather than big-bang event.
//
// - `<Macro>.name: syn::LitStr,`
//       - See upstream issue #142
// - `<Macro>.enter_on_poll: syn::LitBool,`
//       - See upstream issue #133 and https://github.com/tikv/minitrace-rust/issues/126#issuecomment-1077326184
// - `<Macro>.parent: syn::LitStr,`
//       - See upstream issue #117
// - `<Macro>.recorder: syn::Ident,`
//       - See upstream issue #117
// - `<Macro>.recurse: syn::Ident,`
//       - See upstream issue #134
// - `<Macro>.scope: syn::Ident,`
//       - See upstream issue #133 and https://github.com/tikv/minitrace-rust/issues/126#issuecomment-1077326184
// - `<Macro>.variables: syn::Ident,`
//       - See upstream issue #134
// - `<Macro>.conventional: syn::LitBool,`
//       - Benefit is to short circuit some of the parsing logic and hopefully
//         save on compile time - conjecture.
//       - Assume & skip evaluations in analyze when `conventional=true`,
//         and follow these defaults/conventions:
//
//             - name: `fn` name (item). Including path(?)
//             - recorder: `span`
//             - recurse: `None`
//             - scope: `Local` (sync), `Local` (async).
//             - variables: `None`
//             - enter_on_poll:
//               - `None` (sync)
//               - `true` (async) if `false` then convention is that scope: `Threads`.
//
//   Note: These conventions change the current defaults.
//         See https://github.com/tikv/minitrace-rust/issues/126#issuecomment-1077326184
//
//   Current default:
//
//   - `#[trace] async fn` creates thread-safe span (`Span`)
//         - `#[trace(enter_on_poll = true)] async fn` creates local context
//           span (`LocalSpan`)
//   - `#[trace] fn` create local context span (`LocalSpan`)
//
// impl Default for Model {
//
//     fn default() -> Self {
//         Ok(Model {
//             name: todo!(),
//             enter_on_poll: todo!(),
//             parent: todo!(),
//             recorder: todo!(),
//             scope: todo!(),
//             variables: v,
//         })
// }

#[derive(Clone, Copy, Debug, PartialEq, darling::FromMeta)]
pub enum Scope {
    Local,
    Threads,
}

// `Trace` should be moved into `minitrace-macro::validate`.
// Implement `syn::Parse` there, so that in `lib.rs`:
//
//    let attr_args = parse_macro_input!(argsc as crate::trace::validate::TraceAttr);
//    let itemfn = parse_macro_input!(itemc as ItemFn);
//    let args2: proc_macro2::TokenStream = args.clone().into();
//    trace::validate(args2, item.into());
//    let model = trace::analyze(attr_args, itemfn);
//
// becomes
//
//    use crate::trace::validate::Trace;
//    let trace = parse_macro_input!(argsc as Trace);
//    let item = parse_macro_input!(itemc as Trace);
//    let model = trace::analyze(trace, item);
#[derive(
    Clone,
    std::fmt::Debug,
    PartialEq,
    // `darling::FromMeta,` adds two functions:
    //
    // ```
    // fn from_list(items: &[NestedMeta]) -> Result<Trace, syn::Error>
    // ```
    //
    // `try_from_attributes(...)` returns:
    //   - `Ok(None)` if the attribute is missing,
    //   - `Ok(Some(_))` if its there and is valid,
    //   - `Err(_)` otherwise.
    darling::FromMeta,
)]
pub struct Trace {
    // Anything that implements `syn::parse::Parse` is supported.
    #[darling(default)]
    name: Option<syn::LitStr>,
    #[darling(default)]
    scope: Option<Scope>, // Scope::Local, Scope::Thread, etc.

    // Fields wrapped in `Option` are and default to `None` if
    // not specified in the attribute.
    #[darling(default)]
    enter_on_poll: Option<syn::LitBool>,
    #[darling(default)]
    parent: Option<syn::LitStr>,
    #[darling(default)]
    recorder: Option<syn::Ident>,
    #[darling(default)]
    recurse: Option<syn::LitBool>,
    #[darling(default)]
    root: Option<syn::LitBool>,
    #[darling(default)]
    variables: Option<syn::ExprArray>,
    #[darling(default)]
    async_trait: Option<syn::LitBool>,
}

// Produce `Models` (a Vec-newtype)
//
// The `Models` container is built based on the attribute parameters
// held in the `Trace` type.
//
// The inputs are:
// - `meta`: A `syn::Attribute` encapsulated in `TraceAttr`.
// - `items`: A `proc_macr2::TokenStream`.
use syn::visit::Visit;
pub fn analyze(
    //args: std::vec::Vec<syn::NestedMeta>,
    trace: crate::trace::Trace,
    items: proc_macro2::TokenStream,
) -> Models<Model> {
    let mut models = Models::<Model>::new();

    // Prepare and merge each ItemFn with its trace settings
    let tree: syn::File = syn::parse2(items).unwrap();
    let mut visitor = FnVisitor {
        functions: Vec::new(),
    };
    visitor.visit_file(&tree);
    for f in visitor.functions {
        let item_fn = (*f).clone();
        let default_name = item_fn.sig.ident.to_string();
        let _async_fn = match item_fn.sig.asyncness {
            Some(_) => Some(syn::LitBool::new(true, proc_macro2::Span::call_site())),
            None => Some(syn::LitBool::new(false, proc_macro2::Span::call_site())),
        };
        let traced_item = if let crate::trace::Trace {
            default: _,
            validated: _,
            name,
            scope: Some(scope),
            enter_on_poll,
            parent: Some(parent),
            recorder: Some(recorder),
            recurse: Some(recurse),
            root: Some(root),
            variables: Some(variables),
            async_trait: Some(async_trait),
            async_fn: Some(async_fn),
        } = trace.clone()
        {
            // Use default name when no name is passed in.
            // NOTE:
            //     `#[trace(key = "value")]` maps to
            //     `#[trace(name = "__default", key = "value")]`
            let span_name = if name.value() == "__default" {
                syn::LitStr::new(&default_name, proc_macro2::Span::call_site())
            } else {
                name
            };

            TracedItem {
                name: span_name,
                scope,
                enter_on_poll,
                parent,
                recorder,
                recurse,
                root,
                variables,
                async_trait,
                async_fn,
                item_fn,
            }
        } else {
            TracedItem {
                ..Default::default()
            }
        };
        models.push(Model::Item(Box::new(traced_item)));
    }
    models
}

// `Models` are a Vec-newtype
//
// A wrapper that allows us to implement *any* trait. As a new-type it rescinds
// the orphan rule so we have headroom.  Further we can encapsulate or expose
// Vector functionality as we require.
//
// The [`From`] trait provides these conveniences (`match` branch):
//
//     Err(err) => return err.into_compile_error().into(),
//
// Below the following traits are implemented:
//
// - `Debug` (via `#[derive(...)]`)
// - `Default`
// - `Deref`
// - `DerefMut`
// - `Display`
#[derive(Debug, Clone, PartialEq)]
pub struct Models<T>(Vec<T>);

impl<T: std::fmt::Debug> Models<T> {
    pub fn new() -> Models<T> {
        Models(Vec::<T>::new())
    }

    #[allow(dead_code)]
    pub fn with_capacity(capacity: usize) -> Models<T> {
        Models(Vec::<T>::with_capacity(capacity))
    }
}

impl<T: std::fmt::Debug> Default for Models<T> {
    fn default() -> Models<T> {
        Models::new()
    }
}

impl<T: std::fmt::Debug> std::fmt::Display for Models<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl<T: std::fmt::Debug> std::ops::Deref for Models<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}

impl<T: std::fmt::Debug> std::ops::DerefMut for Models<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TracedItem {
    // These are the fields parsed as AttributeArgs into the `Trace` struct
    pub name: syn::LitStr,
    pub scope: crate::trace::parse::Scope, // Scope::Local, Scope::Thread, etc.
    pub enter_on_poll: syn::LitBool,
    pub parent: syn::LitStr,
    pub recorder: syn::Ident,
    pub recurse: syn::LitBool,
    pub root: syn::LitBool,
    pub variables: syn::ExprArray,
    pub async_trait: syn::LitBool,
    pub async_fn: syn::LitBool,

    // `item_fn` pairs each function with the `#[trace(...)]` settings.
    // This structure admits the `recurse=true` option contemplated in issue #134
    pub item_fn: syn::ItemFn,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("Validation logic error")]
pub enum Model {
    Attribute(Trace),
    // Boxed to satisfy clippy::large-enum-variant which is triggered by CI settings
    Item(Box<TracedItem>),
}

// The FnVisitor is used to populate `Models` (a Vec-newtype) when
// `#[trace(recurse=all|public|private)]` on a function or, eventually,
// a module.
struct FnVisitor<'ast> {
    functions: Vec<&'ast syn::ItemFn>,
}

impl<'ast> syn::visit::Visit<'ast> for FnVisitor<'ast> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.functions.push(node);
        // Delegate to the default impl to visit any nested functions.
        syn::visit::visit_item_fn(self, node);
    }
}

// Needed when we do convenient things like this (`match` branch):
//
//     Err(err) => return err.into_compile_error().into(),
//
impl std::convert::From<proc_macro2::TokenStream> for Model {
    fn from(_inner: proc_macro2::TokenStream) -> Model {
        let attribute = Default::default();
        Model::Attribute(attribute)
    }
}

// In the model of the `#[trace]` proc-macro-attribute, the attribute data
// only appears once.  We can have multiple `syn::Item` entries.
// For example:
//
impl std::convert::From<proc_macro2::TokenStream> for Models<Model> {
    fn from(_inner: proc_macro2::TokenStream) -> Models<Model> {
        let attribute = Default::default();
        let mut models = Models::<Model>::new();
        models.push(Model::Attribute(attribute));
        models
    }
}

impl Default for Trace {
    fn default() -> Self {
        // let scope = proc_macro2::Ident::new("Local", proc_macro2::Span::call_site());
        // Some(syn::LitBool::new(false, proc_macro2::Span::call_site()));
        let name = Some(syn::LitStr::new(
            "__default",
            proc_macro2::Span::call_site(),
        ));
        let scope = Some(Scope::Local);
        let enter_on_poll = Some(syn::LitBool::new(false, proc_macro2::Span::call_site()));
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

        Self {
            name,
            async_trait,
            enter_on_poll,
            parent,
            recorder,
            recurse,
            root,
            scope,
            variables,
        }
    }
}

impl Default for TracedItem {
    fn default() -> Self {
        // let scope = proc_macro2::Ident::new("Local", proc_macro2::Span::call_site());
        // Some(syn::LitBool::new(false, proc_macro2::Span::call_site()));
        let name = syn::LitStr::new("__default", proc_macro2::Span::call_site());
        let scope = crate::trace::parse::Scope::Local;
        let enter_on_poll = syn::LitBool::new(false, proc_macro2::Span::call_site());
        let item_fn: syn::ItemFn = syn::parse_quote!(
            fn __default() {}
        );
        let recorder = proc_macro2::Ident::new("span", proc_macro2::Span::call_site());
        let recurse = syn::LitBool::new(false, proc_macro2::Span::call_site());
        let root = syn::LitBool::new(false, proc_macro2::Span::call_site());
        let variables = syn::parse_quote!([]);
        let parent = syn::LitStr::new("__default", proc_macro2::Span::call_site());
        let async_trait = syn::LitBool::new(false, proc_macro2::Span::call_site());
        let async_fn = syn::LitBool::new(false, proc_macro2::Span::call_site());

        Self {
            name,
            async_trait,
            async_fn,
            enter_on_poll,
            item_fn,
            parent,
            recorder,
            recurse,
            root,
            scope,
            variables,
        }
    }
}

#[cfg(test)]
mod tests {
    use syn::Attribute;

    use super::*;

    use crate::trace::analyze::Model;
    use crate::trace::analyze::Models;

    #[test]
    fn models_are_cloneable() {
        let models = Models::<Model>::new();
        let clones = models.clone();
        assert_eq!(models, clones);
    }
    #[test]
    fn with_traces() {
        // `#[trace]`
        //let args: Vec<syn::NestedMeta> = vec![];
        let trace = crate::trace::Trace {
            ..Default::default()
        };

        let items: proc_macro2::TokenStream = syn::parse_quote!(
            #[trace]
            fn f(x: bool) {}
        );
        let models = analyze(trace, items.clone());

        let model = (*models.get(0).unwrap()).clone();
        let traced_item = if let Model::Item(ti) = model {
            Ok((*ti).clone())
        } else {
            Err(())
        }
        .unwrap();
        let expected = TracedItem {
            name: syn::LitStr::new("f", proc_macro2::Span::call_site()),
            item_fn: syn::parse2::<syn::ItemFn>(items).unwrap(),
            ..Default::default()
        };
        assert_eq!(traced_item, expected);
    }

    #[test]
    fn with_trace() {
        // `#[trace]`
        //let args: Vec<syn::NestedMeta> = vec![];
        let trace = crate::trace::Trace {
            ..Default::default()
        };

        let items: proc_macro2::TokenStream = syn::parse_quote!(
            fn f(x: bool) {}
        );
        let models = analyze(trace, items.clone());

        let model = (*models.get(0).unwrap()).clone();
        let traced_item = if let Model::Item(ti) = model {
            Ok((*ti).clone())
        } else {
            Err(())
        }
        .unwrap();
        let expected = TracedItem {
            name: syn::LitStr::new("f", proc_macro2::Span::call_site()),
            item_fn: syn::parse2::<syn::ItemFn>(items).unwrap(),
            ..Default::default()
        };
        assert_eq!(traced_item, expected);
    }

    // There is no filtering/validation in the `analyze` function.
    // All such checks are done in `validate` function.
    #[test]
    fn others_with_traces() {
        // `#[trace]`
        //let args: Vec<syn::NestedMeta> = vec![];
        let trace = crate::trace::Trace {
            ..Default::default()
        };
        let models = analyze(
            trace,
            quote::quote!(
                #[a]
                #[trace]
                #[b]
                fn f(x: bool) -> bool {
                    x
                }
            ),
        );
        let expected: &[Attribute] = &[
            syn::parse_quote!(#[a]),
            syn::parse_quote!(#[trace]),
            syn::parse_quote!(#[b]),
        ];
        let model = (*models.get(0).unwrap()).clone();
        let traced_item = if let Model::Item(item) = model {
            *item.clone()
        } else {
            return;
        };
        let TracedItem { item_fn, .. } = traced_item;
        assert_eq!(expected, item_fn.attrs);
    }

    #[test]
    fn others_with_no_trace() {
        // `#[trace]`
        //let args: Vec<syn::NestedMeta> = vec![];
        let trace = crate::trace::Trace {
            ..Default::default()
        };

        let models = analyze(
            trace,
            syn::parse_quote!(
                #[a]
                #[b]
                fn f(x: bool) {}
            ),
        );
        let expected: &[Attribute] = &[syn::parse_quote!(#[a]), syn::parse_quote!(#[b])];
        let model = (*models.get(0).unwrap()).clone();
        let traced_item = if let Model::Item(item) = model {
            *item.clone()
        } else {
            return;
        };
        let TracedItem { item_fn, .. } = traced_item;
        assert_eq!(expected, item_fn.attrs);
    }
}
