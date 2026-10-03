//! Compile-fail pins: memory a caller did not read, a commit the provider
//! did not make, and a cost nobody measured cannot be built; an unavailable
//! recall cannot be matched away as an empty one.

#[test]
fn memory_outcomes_cannot_be_fabricated() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
