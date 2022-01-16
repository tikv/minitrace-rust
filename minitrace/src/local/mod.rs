// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! Non thread-safe span with low overhead.

pub(crate) mod local_collector;
pub(crate) mod local_parent_guard;
pub(crate) mod local_span;
pub(crate) mod local_span_line;
pub(crate) mod raw_span;
pub(crate) mod span_id;
pub(crate) mod span_queue;

pub use self::local_collector::{LocalCollector, LocalSpans};
pub use self::local_parent_guard::LocalParentGuard;
pub use self::local_span::LocalSpan;
