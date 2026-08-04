//! The rung-props.md refusals that the crate docs demonstrate, pinned as diagnostics.
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
    // rung-props.md G2. Cited by the `constant-arrow-hazard`,
    // `self-governing-not-self-closing`, `disposition-is-a-ruling`, and
    // `no-amending-disposition` rows of docs/conformance.md.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_g2_external_construction.rs");
}

#[test]
fn dropping_a_verdict_under_deny_must_use_is_an_error() {
    // rung-props.md G4.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_g4_must_use_drop.rs");
}

#[test]
fn a_recoverable_verdict_cannot_declare_a_payload() {
    // rung-props.md §2 rule 3 (the payload extension).
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule3_recoverable_payload.rs");
}

#[test]
fn a_continue_arm_target_must_be_a_declared_rung() {
    // rung-props.md §2 rule 3 (the continue-arm extension).
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule3_continue_target.rs");
}

#[test]
fn a_failed_source_rung_must_be_declared() {
    // rung-props.md §2 rule 8 (the error-path extension).
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule8_failed_source.rs");
}

// ── the rest of §2, so that every rule is a case rather than a sentence ─────
//
// rung-props.md §2 states ten rules and calls them a conjunction. Until these
// landed, three of the ten had a `trybuild` snapshot and seven were prose: the
// macro implemented them, and nothing would have noticed if it stopped. A rule
// with no case is a rule the suite cannot tell from a rule that was deleted.

#[test]
fn a_duplicate_carry_field_is_refused() {
    // rung-props.md §2 rule 1.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule1_duplicate_carry.rs");
}

#[test]
fn a_recover_target_must_be_a_declared_rung() {
    // rung-props.md §2 rule 3 (the recover-target branch). Rule 2 — a
    // transition naming an undeclared `from`/`to` rung — has no case because
    // the grammar makes it unreachable: every rung of the spine is declared by
    // the hop that introduces it.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule3_recover_target.rs");
}

#[test]
fn a_recoverable_verdict_without_a_recover_edge_is_refused() {
    // rung-props.md §2 rule 4, and G7's first direction.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule4_missing_recover_edge.rs");
}

#[test]
fn a_recover_edges_target_must_be_a_declared_rung() {
    // rung-props.md §2 rule 5 (the target branch). The other clause — an edge
    // with no matching recover function — is unreachable through the grammar:
    // one `recover { name: V => R }` entry pushes the edge and the function
    // together, so they cannot come apart.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule5_recover_edge_target.rs");
}

#[test]
fn a_terminal_verdict_may_not_carry_a_recover_edge() {
    // rung-props.md §2 rule 6, and G7's terminal clause.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule6_terminal_recover.rs");
}

#[test]
fn a_recover_edge_must_name_a_declared_verdict() {
    // rung-props.md §2 rule 7.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule7_unknown_verdict.rs");
}

#[test]
fn an_impl_body_that_names_no_transition_is_refused() {
    // rung-props.md §2 rule 9 — no phantom bodies.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule9_phantom_body.rs");
}

#[test]
fn an_impl_block_missing_a_body_is_refused() {
    // rung-props.md §2 rule 10 — no gaps.
    trybuild::TestCases::new().compile_fail("tests/ui/spec_rule10_missing_body.rs");
}
