mod analyze;
mod generate;
mod lower;
mod parse;

// Re-export crate::trace::validate::validate(...) as crate::trace::validate(...)
pub use crate::trace::analyze::analyze;
pub use crate::trace::generate::generate;
pub use crate::trace::lower::lower;
pub use crate::trace::lower::quotable::Quotable;
pub use crate::trace::lower::quotable::Quotables;
pub use crate::trace::lower::quotable::Quote;
pub use crate::trace::parse::Trace;
