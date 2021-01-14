# Minitrace
[![Actions Status](https://github.com/tikv/minitrace-rust/workflows/CI/badge.svg)](https://github.com/tikv/minitrace-rust/actions)
[![LICENSE](https://img.shields.io/github/license/tikv/minitrace-rust.svg)](https://github.com/tikv/minitrace-rust/blob/master/LICENSE)

A high-performance, ergonomic timeline tracing library for Rust.


## Concepts

### Span

  A `Span` represents an individual unit of work done. It contains:
  - An operation name
  - A start timestamp and finish timestamp
  - A set of key-value properties

### Local Span Guard

  A `LocalSpanGuard` is used to record a `Span`. Its creation means a `Span`'s begin and its destruction means a `Span`'s
  end.
  
  A `LocalSpanGuard` is thread-local and can be created via method `Span::enter()`.

  *Note: The relation between `Span`s is constructed implicitly. Even within a deeply nested function calls, the inner
  `LocalSpanGuard` can automatically figure out its parent without explicitly passing any tracing context as a parameter.*

### Scope

  A `Scope` is used to trace the execution of a task.
  
  `Scope` is thread-safe so it's okay to send or access across threads. Cloning a `Scope` will produce another `Scope` 
  which will trace the same task. After dropping all `Scope`s related to a task, a `Span`, representing the whole execution
  of the task, will be recorded.

  A new `Scope` can be created via functions `Scope::root()`, `Scope::from_parent()` and `Scope::from_parents()`.

### Collector

  A `Collector` associated to a root `Scope` is used to collect all spans related to a request.


## Usage

```toml
[dependencies]
minitrace = { git = "https://github.com/tikv/minitrace-rust.git" }
```

### Record a Span

To record a common span:
```rust
use minitrace::*;

let _span_guard = Span::enter("my event");
```

To add properties:

```rust
use minitrace::*;

// add a property for a span
let _span_guard = Span::enter("my event").with_property(|| ("key", String::from("value")));

// or add multiple properties for a span
let _span_guard = Span::enter("my event").with_properties(|| {
    vec![
        ("key1", String::from("value1")),
        ("key2", String::from("value2")),
    ]
});
```

###  Synchronous Example

A common pattern to trace synchronous code:

- Create a root `Scope` and a `Collector` via `Scope::root()`, then attach the `Scope` to the current thread.
- Add `Span::enter()`s somewhere, e.g. at the beginning of a code scope, at the beginning of a function, to record spans.
- Make sure the root `Scope` and all guards are dropped, then call `Collector`'s `collect` to get all `Span`s.


```rust
use minitrace::*;

let collector = {
    let (root_scope, collector) = Scope::root("root");
    let _scope_guard = root_scope.enter();

    let _span_guard = Span::enter("child");

    // do something ...

    collector
};

let spans: Vec<Span> = collector.collect();
```

### Asynchronous Example

To trace asynchronous code, we usually transmit `Scope` from one thread to another thread.

#### Threads

```rust
use minitrace::*;

let collector = {
    let (root_scope, collector) = Scope::root("task1");
    let _scope_guard = root_scope.enter();

    let _span_guard = Span::enter("span of task1");
    
    // To trace a child task
    let scope = Scope::from_local_parent("task2");
    std::thread::spawn(move || {
        let _scope_guard = scope.enter();

        let _span_guard = Span::enter("span of also task2");
    });

    collector
};

let spans: Vec<Span> = collector.collect();
```

#### Futures

We provide two `Future` adaptors:

- `in_new_span`: call `Span::enter` at every poll
- `with_scope`: wrap the `Future` with the `Scope`, then call `Scope::try_enter` at every poll

The `with_scope` adaptor is commonly used on a `Future` submitting to a runtime.

```rust
use minitrace::*;

let collector = {
    let (root_scope, collector) = Scope::root("root");
    let _scope_guard = root_scope.enter();

    // To trace another task
    runtime::spawn(async {
        let _ = async {
            // some works
        }.in_new_span("");
    }.with_scope(Scope::from_local_parent("new task")));

    collector
};

let spans: Vec<Span> = collector.collect();
```

### Macros

We provide two macros to help reduce boilerplate code:

- trace
- trace_async

For normal functions, you can change:
```rust
use minitrace::*;

fn amazing_func() {
    let _span_guard = Span::enter("wow");

    // some works
}
```
to
```rust
use minitrace::*;
use minitrace_macro::trace;

#[trace("wow")]
fn amazing_func() {
    // some works
}
```

For async functions, you can change:
```rust
use minitrace::*;

async fn amazing_async_func() {
    async {
        // some works
    }
    .in_new_span("wow")
    .await
}
```
to
```rust
use minitrace::*;
use minitrace_macro::trace_async;

#[trace_async("wow")]
async fn amazing_async_func() {
    // some works
}
```

To access these macros, a dependency should be added as:

```toml
[dependencies]
minitrace-macro = { git = "https://github.com/tikv/minitrace-rust.git" }
```

## User Interface

We support visualization provided by an amazing tracing platform [Jaeger](https://www.jaegertracing.io/).

To experience, a dependency should be added as:
               
```toml
[dependencies]
minitrace-jaeger = { git = "https://github.com/tikv/minitrace-rust.git" }
```

### Report to Jaeger

```rust
use minitrace_jaeger::Reporter;

let spans = /* collect from a collector */;

let socket = SocketAddr::new("127.0.0.1:6831".parse().unwrap());
let reporter = Reporter::new(socket, "service name");

const TRACE_ID: u64 = 42;
const SPAN_ID_PREFIX: u32 = 42;
const ROOT_PARENT_SPAN_ID: u64 = 0;
reporter.report(TRACE_ID, SPAN_ID_PREFIX, ROOT_PARENT_SPAN_ID, &spans).expect("report error");
```

### Setup Jaeger
```sh
docker run --rm -d -p6831:6831/udp -p16686:16686 --name jaeger jaegertracing/all-in-one:latest
```

### Run Synchronous Example

```sh
cargo run --example synchronous
```

Open http://localhost:16686 to see the results.

![Jaeger Synchronous](img/jaeger-synchronous.png)

### Run Asynchronous Example

```sh
cargo run --example asynchronous
```

Open http://localhost:16686 to see the results.

![Jaeger Asynchronous](img/jaeger-asynchronous.png)
