//! The SPEC.md refusals that the crate docs demonstrate, pinned as diagnostics.
//!
//! Each case here has a twin: a `compile_fail` doctest in `rung/src/lib.rs`
//! that shows the same refusal in rustdoc, where a reader will meet it. The
//! doctests stay — they are the documentation. They are not the assertion.
//!
//! rustdoc does **not** verify the error code on a `compile_fail` block. A
//! block annotated `compile_fail,E0999` passes, and E0999 does not exist. So a
//! `compile_fail` doctest asserts exactly one thing — "this did not compile" —
//! and cannot distinguish the refusal it was written for from a typo, a missing
//! import, or a name that went out of scope when rustdoc wrapped the snippet in
//! `fn main`. Every case below has been observed failing for a *different*
//! reason than its prose claimed, or is one edit away from it.
//!
//! `trybuild` diffs the full rendered stderr against a committed `.stderr`
//! snapshot, so the error code and the message text are both part of the
//! assertion. A case that starts failing for a different reason is a diff, not
//! a pass — and a case that stops failing is a failure.
//!
//! The `.stderr` files are the committed statement of *why* each case must
//! fail. Regenerate deliberately (`TRYBUILD=overwrite`), never reflexively.

#[test]
fn external_construction_of_a_mid_ladder_rung_is_e0624() {
    // SPEC.md G2. Cited by the `constant-arrow-hazard`,
    // `self-governing-not-self-closing`, `disposition-is-a-ruling`, and
    // `no-amending-disposition` rows of docs/conformance.md.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_g2_external_construction.rs");
}

#[test]
fn dropping_a_verdict_under_deny_must_use_is_an_error() {
    // SPEC.md G4.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_g4_must_use_drop.rs");
}

#[test]
fn a_recoverable_verdict_cannot_declare_a_payload() {
    // SPEC.md §2 rule 3 (the payload extension).
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule3_recoverable_payload.rs");
}

#[test]
fn a_continue_arm_target_must_be_a_declared_rung() {
    // SPEC.md §2 rule 3 (the continue-arm extension).
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule3_continue_target.rs");
}

#[test]
fn a_failed_source_rung_must_be_declared() {
    // SPEC.md §2 rule 8 (the error-path extension).
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule8_failed_source.rs");
}
