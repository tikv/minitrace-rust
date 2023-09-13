pub struct CollectLifetimes {
    pub elided: Vec<syn::Lifetime>,
    pub explicit: Vec<syn::Lifetime>,
    pub name: &'static str,
    pub default_span: proc_macro2::Span,
}

impl CollectLifetimes {
    pub fn new(name: &'static str, default_span: proc_macro2::Span) -> Self {
        CollectLifetimes {
            elided: Vec::new(),
            explicit: Vec::new(),
            name,
            default_span,
        }
    }

    fn visit_opt_lifetime(&mut self, lifetime: &mut Option<syn::Lifetime>) {
        match lifetime {
            None => *lifetime = Some(self.next_lifetime(None)),
            Some(lifetime) => self.visit_lifetime(lifetime),
        }
    }

    fn visit_lifetime(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident == "_" {
            *lifetime = self.next_lifetime(lifetime.span());
        } else {
            self.explicit.push(lifetime.clone());
        }
    }

    fn next_lifetime<S: Into<Option<proc_macro2::Span>>>(&mut self, span: S) -> syn::Lifetime {
        let name = format!("{}{}", self.name, self.elided.len());
        let span = span.into().unwrap_or(self.default_span);
        let life = syn::Lifetime::new(&name, span);
        self.elided.push(life.clone());
        life
    }
}

impl syn::visit_mut::VisitMut for CollectLifetimes {
    fn visit_receiver_mut(&mut self, arg: &mut syn::Receiver) {
        if let Some((_, lifetime)) = &mut arg.reference {
            self.visit_opt_lifetime(lifetime);
        }
    }

    fn visit_type_reference_mut(&mut self, ty: &mut syn::TypeReference) {
        self.visit_opt_lifetime(&mut ty.lifetime);
        syn::visit_mut::visit_type_reference_mut(self, ty);
    }

    fn visit_generic_argument_mut(&mut self, gen: &mut syn::GenericArgument) {
        if let syn::GenericArgument::Lifetime(lifetime) = gen {
            self.visit_lifetime(lifetime);
        }
        syn::visit_mut::visit_generic_argument_mut(self, gen);
    }
}
