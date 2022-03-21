#[test]
fn spans() {
    let t = trybuild::TestCases::new();
    t.pass("tests/spans/*.rs");
}
