// Function naming convention: `<environment>()`
// Where:
// - `environment`:
//   - `indev`(in-development): When iterating on a single test case,
//              edit the function to point to the test case in-development.
//   - `dev`: Generate all missing `*.expanded.rs` files, and flag changes.
//   - `ci`: Generate nothing, and fail on mismatches.
//
#[test]
#[cfg(not(feature = "ci"))]
pub fn indev() {
    // To generate macro result files
    macrotest::expand("integration/tests/issues/non-drop-local.rs");
    build_indev();
}

fn build_indev() {
    let t = trybuild::TestCases::new();
    t.pass("integration/tests/issues/*.expanded.rs");
}

#[test]
#[cfg(not(feature = "ci"))]
pub fn dev() {
    // To generate macro result files
    macrotest::expand("integration/tests/issues/*.rs");
    build_dev();
}

fn build_dev() {
    let t = trybuild::TestCases::new();
    t.pass("src/build/issues/*.expanded.rs");
}

#[test]
#[cfg(feature = "ci")]
pub fn ci() {
    // To test generated macro result files
    macrotest::expand_without_refresh("tests/expand/issues/*.rs");
    build_ci();
}

fn build_ci() {
    let t = trybuild::TestCases::new();
    t.pass("tests/expand/issues/*.expanded.rs");
}
