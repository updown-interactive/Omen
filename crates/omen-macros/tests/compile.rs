#[test]
fn macro_compilation() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/pass/*.rs");
    tests.compile_fail("tests/fail/*.rs");
}
