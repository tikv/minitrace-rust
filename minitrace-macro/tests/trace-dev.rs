// Useful while working on specific test cases
#[test]
#[ignore]
fn trace_err_dev() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trace/ui/err/006-has-too-many-arguments.rs");
}
#[test]
#[ignore]
fn trace_ok_dev() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trace/ui/ok/00-has-no-arguments.rs");
}
