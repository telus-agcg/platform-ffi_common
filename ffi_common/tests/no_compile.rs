#[test]
fn should_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/no_compile/*.rs");
}
