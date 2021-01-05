use std::cell::RefCell;

use crate::local::observer::Observer;
use crate::Scope;
use std::sync::Arc;

thread_local! {
    static LOCAL_SCOPE: RefCell<Option<LocalScope>> = RefCell::new(None);
}

pub struct LocalScope {
    scope: Scope,
    observer: Option<Observer>,
}

impl LocalScope {
    pub fn with_local_scope<R>(f: impl FnOnce(Option<&mut Scope>) -> R) -> R {
        LOCAL_SCOPE.with(|local_scope| {
            let mut local_scope = local_scope.borrow_mut();
            f(local_scope.as_mut().map(|ls| &mut ls.scope))
        })
    }
}

pub struct LocalScopeGuard;
impl !Send for LocalScopeGuard {}
impl !Sync for LocalScopeGuard {}

impl Drop for LocalScopeGuard {
    fn drop(&mut self) {
        LOCAL_SCOPE.with(|local_scope| {
            if let Some(LocalScope {
                scope,
                observer: Some(observer),
            }) = local_scope.borrow_mut().take()
            {
                let raw_spans = Arc::new(observer.collect());
                scope.submit_raw_spans(raw_spans);
            }
        })
    }
}

impl LocalScopeGuard {
    #[inline]
    pub fn new(scope: Scope) -> Self {
        Self::new_with_observer(scope, None)
    }

    #[inline]
    pub fn new_with_observer(scope: Scope, observer: Option<Observer>) -> Self {
        LOCAL_SCOPE.with(|local_scope| {
            let mut local_scope = local_scope.borrow_mut();

            if local_scope.is_some() {
                panic!("Attach too much scopes: > 1")
            }

            *local_scope = Some(LocalScope { scope, observer })
        });

        LocalScopeGuard
    }

    #[inline]
    pub fn detach(self) -> Scope {
        LOCAL_SCOPE.with(|local_scope| {
            let LocalScope { scope, observer } = local_scope.borrow_mut().take().unwrap();

            if let Some(observer) = observer {
                let raw_spans = Arc::new(observer.collect());
                scope.submit_raw_spans(raw_spans);
            }

            scope
        })
    }
}
