// Useful while working on specific test cases
#[test]
#[ignore]
// Integration tests to be migrated once PR #127 lands and closes #113
fn trace_err_dev() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trace/ui/err/006-has-too-many-arguments.rs");
}
#[test]
#[ignore]
// Integration tests to be migrated once PR #127 lands and closes #113
fn trace_ok_dev() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trace/ui/ok/00-has-no-arguments.rs");
}
