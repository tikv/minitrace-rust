#[test]
#[ignore]
// Integration tests to be migrated once PR #127 lands and closes #113
pub fn expand_defaults_dev() {
    // To generate macro result files
    macrotest::expand("tests/expand/defaults/*.rs");
}

#[test]
#[cfg(feature = "ci")]
#[ignore]
// Integration tests to be migrated once PR #127 lands and closes #113
pub fn expand_defaults_ci() {
    // To test generated macro result files
    macrotest::expand_without_refresh("tests/expand/defaults/*.rs");
}

#[test]
#[ignore]
// Integration tests to be migrated once PR #127 lands and closes #113
pub fn expand_non_defaults_dev() {
    // To generate macro result files
    macrotest::expand("tests/expand/non-defaults/*.rs");
}

#[test]
#[cfg(feature = "ci")]
#[ignore]
// Integration tests to be migrated once PR #127 lands and closes #113
pub fn expand_non_defaults_ci() {
    // To test generated macro result files
    macrotest::expand_without_refresh("tests/expand/non-defaults/*.rs");
}

#[test]
#[ignore]
pub fn expand_issue_001_dev() {
    // To generate macro result files
    macrotest::expand_args(
        "tests/expand/issues/tokio-1615.rs",
        &["--manifest-path", "./Cargo.toml"],
    );
    build_issues_dev();
}

#[cfg(not(feature = "ci"))]
fn build_issues_dev() {
    let t = trybuild::TestCases::new();
    t.pass("tests/expand/issues/*.expanded.rs");
}

#[test]
#[ignore]
#[cfg(feature = "ci")]
pub fn issues_ci() {
    // To test generated macro result files
    macrotest::expand_without_refresh("tests/expand/issues/*.rs");
    build_issues_ci();
}

#[cfg(feature = "ci")]
fn build_issues_ci() {
    let t = trybuild::TestCases::new();
    t.pass("tests/expand/issues/*.expanded.rs");
}
