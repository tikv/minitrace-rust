// Function naming convention: `<environment>()`
// Where:
// - `environment`:
//   - `indev`: When iterating on a single test case; edit the function to
//     point to the test case in development.
//   - `dev`: Generate all missing `*.expanded.rs` files, and flag changes.
//   - `ci`: Generate nothing, and fail on mismatches.
//
use super::IntegrationTest;
use crate::tests::*; //IntegrationTest;

inventory::submit!(IntegrationTest {
    name: "indev",
    test_fn: indev,
    indev: Some(true),
});

#[cfg(not(feature = "ci"))]
pub fn indev() {
    // To generate macro result files
    let src = "integration/tests/defaults/no-be-no-drop-local.rs";
    let srcx = "integration/tests/defaults/no-be-no-drop-local.expanded.rs";

    #[cfg(feature = "as")]
    macrotest::expand_args(
        src,
        &[
            "--features",
            "minitrace-tests/default minitrace-tests/as",
            "--manifest-path",
            "./Cargo.toml",
        ],
    );
    #[cfg(feature = "tk")]
    macrotest::expand_args(
        src,
        &[
            "--features",
            "minitrace-tests/default minitrace-tests/tk",
            "--manifest-path",
            "./Cargo.toml",
        ],
    );

    build_indev(srcx);
}

fn build_indev(src: &str) {
    let t = trybuild::TestCases::new();
    t.pass(src);
}

#[cfg(not(feature = "ci"))]
pub fn dev() {
    // To generate macro result files
    macrotest::expand("integration/tests/defaults/*.rs");
    build_dev();
}

fn build_dev() {
    let t = trybuild::TestCases::new();
    t.pass("integration/tests/defaults/*.expanded.rs");
}

// #[test]
#[cfg(feature = "ci")]
pub fn ci() {
    // To test generated macro result files
    macrotest::expand_without_refresh("src/tests/expand/defaults/*.rs");
    build_ci();
}

fn build_ci() {
    let t = trybuild::TestCases::new();
    t.pass("scr/tests/expand/defaults/*.expanded.rs");
}
