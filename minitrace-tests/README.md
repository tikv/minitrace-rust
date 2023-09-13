# minitrace-tests

Minitrace attribute `#[trace]` integration tests for developers and
CI environments.

NOTE:
Cargo virtual manifests do not support the `[features]` stanza (without also
having the `[package stanza]`).  See [Cargo issue 4942](https://github.com/rust-lang/cargo/issues/4942).
There are several Cargo issues around features that mean it is a
fraught exercise to change this (working) test harness - here be dragons.

We aren't the first to bang our heads on this:

***
Thanks to the following for [how to do this](https://www.infinyon.com/blog/2021/04/rust-custom-test-harness/)

- [Infinyon](https://www.infinyon.com) .
- [Fluvio project](https://github.com/infinyon/fluvio)

***

## Developing against a test

1. Add the test file, say `tests/issues/nnn.rs`.
2. Point the in-development test runner to this file:

    ```rust
    // minitrace-tests/src/tests/issues.rs
    pub fn indev() {
        // To generate macro result files
        macrotest::expand("src/tests/issues/nnn.rs");
        build_indev();
    }
    ```

3. Implement the required logic.
4. Run the single `indev` test case:

    ```bash
    cargo test --features "default minitrace-tests/tk" --manifest-path minitrace-tests/Cargo.toml \
               indev-tokio \
               -- --nocapture
    ```

    or

    ```bash
    cd minitrace-tests
    cargo test build::issues::indev -- --nocapture
    ```

5. Iterate 3) and 4) until green.

## Adding a test

Scenario:
Add several test cases (developed as above) in the issues category
`minitrace-tests/src/tests/issues`

1. Add the test file, say `build/issues/nnn.rs`.
2. Generate the expanded Rust code:

    ```bash
    cargo test issues-dev-tokio --no-fail-fast &>~/tmp/log.txt
    ```

3. Check the expanded code is as expected: `build/issues/nnn.expanded.rs`
4. Check the log results show the build was successful.
5. commit and push.

## Test suit execution by environment

Scenario:
Run test suite for the issues category `minitrace-tests/src/tests/issues`

To run developer, but not CI-scoped integration tests:

```bash
cargo test issues-dev-tokio --no-fail-fast
```

To run the developer and the CI-scoped tests:

```bash
cargo test issues-ci-tokio \
           --features ci \
           --no-fail-fast
```

## CI/remote test execution

```bash
cargo test --workspace --all-features --no-fail-fast
```
