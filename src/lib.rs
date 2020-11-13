// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![feature(map_first_last)]
#![feature(negative_impls)]

pub use crate::future::FutureExt;
pub use crate::local::scope_guard::LocalScopeGuard;
pub use crate::local::span_guard::LocalSpanGuard;
pub use crate::span::cycle::DefaultClock;
pub use crate::span::span_id::{DefaultIdGenerator, SpanId};
pub use crate::span::Span;
pub use crate::trace::collector::Collector;
pub use crate::trace::scope::Scope;

pub mod collections;

pub(crate) mod future;
pub(crate) mod local;
pub(crate) mod span;
pub(crate) mod trace;

#[inline]
pub fn set_span_id_prefix(id_prefix: u32) {
    DefaultIdGenerator::set_prefix(id_prefix)
}

#[inline]
pub fn start_scope(scope: &Scope) -> LocalScopeGuard {
    LocalScopeGuard::new(scope.acquirer_group.clone())
}

#[inline]
pub fn start_scopes<'a, I: Iterator<Item = &'a Scope>>(iter: I) -> LocalScopeGuard {
    LocalScopeGuard::new_from_scopes(iter)
}

#[inline]
pub fn start_span(event: &'static str) -> LocalSpanGuard {
    LocalSpanGuard::new(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use minitrace_macro::trace;

    fn four_spans() {
        {
            // wide
            for _ in 0..2 {
                let _g =
                    start_span("iter span").with_property(|| ("tmp_property", "tmp_value".into()));
            }
        }

        {
            #[trace("rec span")]
            fn rec(mut i: u32) {
                i -= 1;

                if i > 0 {
                    rec(i);
                }
            }

            // deep
            rec(2);
        }
    }

    #[test]
    fn single_thread_single_scope() {
        let spans = {
            let (root_scope, collector) = Scope::root("root");
            let _sg = start_scope(&root_scope);

            four_spans();

            collector
        }
        .collect(true, None, None);

        assert_eq!(spans.len(), 5);
    }

    #[test]
    fn single_thread_multiple_scopes() {
        let (spans1, spans2, spans3) = {
            let (c1, c2, c3) = {
                let (root_scope1, collector1) = Scope::root("root1");
                let (root_scope2, collector2) = Scope::root("root2");
                let (root_scope3, collector3) = Scope::root("root3");

                let _sg = start_scopes([root_scope1, root_scope2, root_scope3].iter());

                four_spans();

                (collector1, collector2, collector3)
            };

            (
                c1.collect(true, None, None),
                c2.collect(true, None, None),
                c3.collect(true, None, None),
            )
        };

        assert_eq!(spans1.len(), 5);
        assert_eq!(spans2.len(), 5);
        assert_eq!(spans3.len(), 5);
    }

    #[test]
    fn multiple_threads_single_scope() {
        let spans = {
            let (scope, collector) = Scope::root("root");
            let _sg = start_scope(&scope);

            for _ in 0..4 {
                let child_scope = Scope::child("cross-thread");
                std::thread::spawn(move || {
                    let _sg = start_scope(&child_scope);
                    four_spans();
                });
            }

            four_spans();
            collector
        }
        .collect(true, None, None);

        assert_eq!(spans.len(), 25);
    }

    #[test]
    fn multiple_threads_multiple_scopes() {
        let (spans1, spans2) = {
            let (c1, c2) = {
                let (root_scope1, collector1) = Scope::root("root1");
                let (root_scope2, collector2) = Scope::root("root2");

                let _sg = start_scopes([root_scope1, root_scope2].iter());

                for _ in 0..4 {
                    let scope = Scope::child("cross-thread");
                    std::thread::spawn(move || {
                        let _sg = start_scope(&scope);
                        four_spans();
                    });
                }

                four_spans();
                (collector1, collector2)
            };

            (c1.collect(true, None, None), c2.collect(false, None, None))
        };

        assert_eq!(spans1.len(), 25);
        assert_eq!(spans2.len(), 25);
    }

    #[test]
    fn multiple_scopes_without_spans() {
        let (spans1, spans2, spans3) = {
            let (c1, c2, c3) = {
                let (root_scope1, collector1) = Scope::root("root1");
                let (root_scope2, collector2) = Scope::root("root2");
                let (root_scope3, collector3) = Scope::root("root3");

                let _sg1 = start_scopes([root_scope1, root_scope2].iter());
                let _sg2 = start_scope(&root_scope3);

                (collector1, collector2, collector3)
            };

            (
                c1.collect(true, None, None),
                c2.collect(true, None, None),
                c3.collect(true, None, None),
            )
        };

        assert_eq!(spans1.len(), 1);
        assert_eq!(spans2.len(), 1);
        assert_eq!(spans3.len(), 1);
    }
}
