//! Gate markers on `ladder!` transitions — the first half of Q11.
//!
//! A `#[judgmental(Role)]` marker on a rung gives the transition that produces
//! it a second parameter, `Qualified<Role>`, which only `Pool::qualify` mints.
//! That makes the *signature* honest: a judgmental transition cannot be called
//! without an outside, and a decidable one has no parameter an outside could
//! enter through (decidable-cannot-consult-pool). It does **not** make the
//! arrow admissible — see the note at the bottom of this file.
//!
//! The five refusals are `trybuild` cases rather than `compile_fail` doctests
//! on purpose. rustdoc does not verify the error code on a `compile_fail`
//! block — a block annotated `compile_fail,E0999` passes — so a doctest cannot
//! distinguish "failed for the intended reason" from "failed because of a
//! typo". `trybuild` diffs the full stderr against a committed snapshot, so the
//! error code and the message text are both part of the assertion.

use rung::{Pool, Principal, Prov, Provenanced, Qualified, Role, ladder};

// ── a role, a principal, and a pool ─────────────────────────────────────────

#[derive(Clone, Copy)]
struct Reviewer;
impl Role for Reviewer {
    const NAME: &'static str = "reviewer";
}

#[derive(Clone, Copy)]
struct Judge;
impl Role for Judge {
    const NAME: &'static str = "judge";
}

struct Person {
    id: &'static str,
    prov: Prov,
    roles: &'static [&'static str],
}

impl Provenanced for Person {
    fn provenance(&self) -> Prov {
        self.prov.clone()
    }
}

impl Principal for Person {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }
}

// ── the ladder ──────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct SpecData(&'static str);

impl Provenanced for SpecData {
    fn provenance(&self) -> Prov {
        Prov::of([self.0])
    }
}

#[derive(Clone, PartialEq)]
struct LoopState {
    rounds: u32,
}

impl Provenanced for LoopState {
    fn provenance(&self) -> Prov {
        Prov::of(["drafter"])
    }
}

struct Report(u32);

// Both markable positions are marked: the forward transition `active`, and the
// branching transition `step` — `step` is the `dispose` position of Het's pass,
// where a judge rules on what the algebra produced.
ladder!(Review {
    Spec(SpecData)
        => #[judgmental(Reviewer)] Active(LoopState)
        => #[judgmental(Judge)] { Converged(Report) | Stalled => Active }
    recover { unstall: Stalled => Active }
} impl {
    active = |_spec, q| {
        assert_eq!(q.role_name(), "reviewer");
        Active::new(LoopState { rounds: 0 })
    },
    step = |active, q| {
        assert_eq!(q.role_name(), "judge");
        if active.payload.rounds >= 1 {
            Ok(StepOutcome::Converged(Converged::new(Report(active.payload.rounds))))
        } else {
            Ok(StepOutcome::Stalled(Stalled::new(active)))
        }
    },
    unstall = |stalled| {
        let active = stalled.into_source();
        Active::new(LoopState { rounds: active.payload.rounds + 1 })
    },
});

fn pool() -> Pool<Person> {
    Pool::new(vec![Person {
        id: "rita",
        prov: Prov::of(["rita"]),
        roles: &["reviewer", "judge"],
    }])
}

// ── 1. the positive case ────────────────────────────────────────────────────

#[test]
fn judgmental_transition_takes_a_qualified_token() {
    // The signature itself is the assertion. Coercing the generated `fn` item to
    // a `fn` pointer of the exact expected type fails to compile if the second
    // parameter is absent, extra, or of another type — so this line is what goes
    // red if the macro stops emitting the gate parameter.
    let active_fn: fn(review::Spec, Qualified<Reviewer>) -> review::Active = review::active;
    let step_fn: fn(
        review::Active,
        Qualified<Judge>,
    ) -> Result<review::StepOutcome, review::Failed<review::Active>> = review::step;

    let pool = pool();
    let spec = SpecData("drafter");

    // The token is minted, not written: `Pool::qualify` refuses a principal who
    // is not capable of the role, or who shares provenance with the material.
    let licence: Qualified<Reviewer> = pool.qualify(&spec).expect("rita qualifies as reviewer");
    assert_eq!(licence.principal_id(), "rita");

    let active = active_fn(review::Spec::new(spec), licence);
    assert_eq!(active.payload.rounds, 0);

    // Each dispatch re-runs the filter: the licence above was consumed by value.
    let ruling: Qualified<Judge> = pool
        .qualify(&active.payload)
        .expect("rita qualifies as judge");
    let active = match step_fn(active, ruling) {
        Ok(review::StepOutcome::Stalled(stalled)) => review::unstall(stalled),
        _ => panic!("expected a stall on the first round"),
    };

    let ruling: Qualified<Judge> = pool
        .qualify(&active.payload)
        .expect("rita qualifies as judge");
    match step_fn(active, ruling) {
        Ok(review::StepOutcome::Converged(done)) => assert_eq!(done.into_payload().0, 1),
        _ => panic!("expected convergence on the second round"),
    }
}

// ── 1b. the injected prologue (G13) — a body cannot skip the binding ────────
//
// The second half of Q11. G12 makes the *signature* honest; it does not make
// the *arrow* admissible, because a token minted against one argument could be
// spent on another. The macro therefore injects the binding check as a prologue
// — the same discipline it applies to `must_progress` for G8 — so the check
// cannot live in the body and the body cannot skip it.
//
// The ladder below is the adversarial case: its judgmental body **never
// mentions the token**. It binds it to `_q` and returns. Under G12 alone this
// arrow is "judgmental" and discharges nothing at all.

#[derive(Clone, PartialEq)]
struct Draft(&'static str);

impl Provenanced for Draft {
    fn provenance(&self) -> Prov {
        Prov::of([self.0])
    }
}

ladder!(Blind {
    Manuscript(Draft)
        => #[judgmental(Reviewer)] Reviewed(u32)
        => { Filed }
} impl {
    // No `q` in sight. Whatever this body proves, it is not that a qualified
    // outside was consulted about *this* manuscript.
    reviewed = |_manuscript, _q| { Reviewed::new(0) },
    step     = |_reviewed| { Ok(StepOutcome::Filed(Filed::new())) },
});

#[test]
fn a_body_that_ignores_the_token_still_gets_the_binding_check() {
    // The licence is measured against the very manuscript it is spent on.
    let pool = pool();
    let manuscript = Draft("drafter");
    let licence: Qualified<Reviewer> = pool
        .qualify_for(&manuscript)
        .expect("rita is disjoint from the drafter");

    let reviewed = blind::reviewed(blind::Manuscript::new(manuscript), licence);
    assert_eq!(reviewed.payload, 0);
}

#[test]
#[should_panic(expected = "this qualifying token was minted against a different argument")]
fn the_injected_prologue_refuses_a_transferred_token_the_body_never_reads() {
    // Everything here is honestly obtained. `rita` really did pass both filters
    // against `someone-else`'s draft — the token is not forged, and the arrow's
    // signature is satisfied. What it was never measured against is the
    // manuscript it is about to license judgment on.
    let pool = pool();
    let elsewhere = Draft("someone-else");
    let transferred: Qualified<Reviewer> = pool
        .qualify_for(&elsewhere)
        .expect("rita is disjoint from someone-else");

    let manuscript = blind::Manuscript::new(Draft("drafter"));

    // The body would accept this without a murmur. The prologue does not.
    let _ = blind::reviewed(manuscript, transferred);
}

// ── 2–6. the refusals ───────────────────────────────────────────────────────
//
// One `trybuild::TestCases` per case, so a failure names the test that broke
// rather than a bundle. Each `.stderr` snapshot is the committed statement of
// *why* the case must fail; a case that starts failing for a different reason
// is a diff, not a pass.

#[test]
fn calling_a_judgmental_transition_without_a_token_is_e0061() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_missing_token.rs");
}

#[test]
fn a_qualified_token_cannot_be_constructed_outside_the_pool() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_forged_token.rs");
}

#[test]
fn judgmental_without_a_role_is_refused() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_judgmental_no_role.rs");
}

#[test]
fn authorial_is_refused_as_not_yet_supported() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_authorial_unsupported.rs");
}

#[test]
fn conditional_is_refused_and_names_the_open_question() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_conditional_unsupported.rs");
}

// ── what these tests do and do not establish ────────────────────────────────
//
// The trybuild cases and the `fn`-pointer coercion are about the *signature*
// (G12): a judgmental transition cannot be called without a token, a decidable
// one has no parameter a token could enter through, and the token cannot be
// forged. `the_injected_prologue_refuses_a_transferred_token_the_body_never_reads`
// is about the *argument* (G13): the token records `π(a)` and the macro-injected
// prologue admits it only there, so a licence earned against one argument
// cannot be spent on another even by a body that never looks at it
// (non-identity-by-construction, disjointness-against-argument).
//
// What is still not established is that (G12 ∧ G13) = gate-faithfulness. Three
// things remain outside: `#[authorial]` and `#[conditional(..)]` are refused
// rather than implemented, so two of Het's four gates have no signature at all;
// the *verdict* a judgmental body returns is unconstrained, so admissibility of
// the returned value (`π(f(a)) ∩ π(a) = ∅`) is a body property and inherits Q1's
// limit whole; and a decidable transition may still reach a clock or a socket,
// because the decidable signature excludes only Het's outside
// (purity-not-secured). The argument, and what would falsify it, is in Q11's
// note under docs/questions/open/.
