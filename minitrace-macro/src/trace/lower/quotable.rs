// `Quotables` a Vec-newtype
//
// A wrapper that allows us to implement the [`From`] trait.
// The [`From`] trait provides these conveniences (`match` branch):
//
//     Err(err) => return err.into_compile_error().into(),
//
// Below the following traits are implemented:
//
// - Debug (via #[derive(...)])
// - Default
// - Deref
// - DerefMut
// - Display
#[derive(Debug, Clone)]
pub struct Quotables<T>(Vec<T>);

impl<T: std::fmt::Debug> Quotables<T> {
    pub fn new() -> Quotables<T> {
        Quotables(Vec::<T>::new())
    }

    #[allow(dead_code)]
    pub fn with_capacity(capacity: usize) -> Quotables<T> {
        Quotables(Vec::<T>::with_capacity(capacity))
    }
}

impl<T: std::fmt::Debug> Default for Quotables<T> {
    fn default() -> Quotables<T> {
        Quotables::new()
    }
}

impl<T: std::fmt::Debug> std::fmt::Display for Quotables<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl<T: std::fmt::Debug> std::ops::Deref for Quotables<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}

impl<T: std::fmt::Debug> std::ops::DerefMut for Quotables<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, thiserror::Error)]
#[error("Validation logic error")]
pub enum Quotable {
    Item(Quote),
}

#[derive(Clone, Debug, thiserror::Error)]
pub struct Quote {
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub constness: Option<syn::token::Const>,
    pub unsafety: Option<syn::token::Unsafe>,
    pub abi: Option<syn::Abi>,
    pub ident: syn::Ident,
    pub gen_params: syn::punctuated::Punctuated<syn::GenericParam, syn::Token![,]>,
    pub params: syn::punctuated::Punctuated<syn::FnArg, syn::Token![,]>,
    pub return_type: syn::ReturnType,
    pub where_clause: Option<syn::WhereClause>,
    pub func_body: proc_macro2::TokenStream,
}

impl std::fmt::Display for Quote {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
