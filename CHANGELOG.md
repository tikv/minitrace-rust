# Changelog

## [Next version]

- Remove `LocalSpanGuard` and merge it into `LocalSpan`.

## v0.3.1

- Add a async varient of jaeger reporting function `minitrace::report()`.
- `LocalSpan::with_property` now no longer takes `self` but `&mut self` instead.

## v0.3.0

- `Collector::collect()` becomes an async function because the span collection work is moved to a background thread to extremely reduce the performance overhead on the code being tracing.
- Attribute macro `#[trace]` on async function becomes able to automatically extract the local parent in the caller's context. Previously, the caller must manually call `in_span()`.

## v0.2.0

- All API get redesigned for better egnormic experience.
- Attribute macro `#[trace]` automactically detects `async fn` and crate `async-trait`, and since that, `#[trace_async]` is removed.
