//! Compile-fail pins for the turn check's seal and the memory retain seal.

/// `Status::Completed` cannot be built without a `Checked` (or the switched-off
/// witness): the fields that would let a caller fabricate one are private.
#[test]
fn a_completion_cannot_be_fabricated() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
