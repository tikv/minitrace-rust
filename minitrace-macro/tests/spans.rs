#[test]
#[ignore]
// Integration tests to be migrated once PR #127 lands and closes #113
fn spans() {
    let t = trybuild::TestCases::new();
    t.pass("tests/spans/*.rs");
}
