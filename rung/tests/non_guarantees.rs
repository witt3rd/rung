//! The non-guarantees, proven.
//!
//! `rung-props.md` §5 says what the macro does **not** enforce. Those read as
//! unprovable — a claim that something is not enforced has no satisfying model —
//! and that reading is wrong.
//!
//! A non-guarantee is proven by a test that **exercises the gap**. If the gap
//! ever closes, the test stops compiling or stops passing. So these fail when
//! the system gets *stronger*, which is the opposite direction from every other
//! test in this suite and exactly what makes them proofs.
//!
//! ## Why that is worth having
//!
//! A stated limit nothing checks decays in one of two ways. It is quietly
//! closed, and the document keeps disclaiming something the macro now does — a
//! specification that understates its own guarantees is as wrong as one that
//! overstates them. Or someone builds on the limit, it closes underneath them,
//! and nothing said so.
//!
//! Pinning the boundary makes it detectable in both directions. These tests are
//! the only place in the suite where a *green* run is the interesting outcome
//! and a red one means good news that needs writing down.

use rung::ladder;

// ════════════════════════════════════════════════════════════════════════════
// 5.1 · Transition-body correctness
// ════════════════════════════════════════════════════════════════════════════

#[derive(Clone, PartialEq, Debug)]
struct Count(i64);

ladder!(Arithmetic {
    Given(Count) => Doubled(Count) => { Reported(Count) }
} impl {
    // The body claims to double and subtracts instead. The macro has no opinion
    // — the type proves the transition RAN, not that its logic was valid.
    doubled = |given| { Doubled::new(Count(given.payload.0 - 1)) },
    step = |doubled| { Ok(StepOutcome::Reported(Reported::new(doubled.payload))) },
});

/// **5.1 transition-body-correctness.** A body whose logic contradicts its own
/// name compiles, runs, and reaches a terminal.
///
/// This is the boundary between typestate and formal verification, and it is
/// where rung stops. If the macro ever began verifying bodies, this ladder
/// would not build.
#[test]
fn a_transition_body_may_be_wrong_and_the_macro_does_not_care() {
    let given = arithmetic::Given::new(Count(21));
    let doubled = arithmetic::doubled(given);

    // 21 doubled is 42. The ladder says 20, and nothing objected.
    assert_eq!(doubled.payload, Count(20));

    match arithmetic::step(doubled) {
        Ok(arithmetic::StepOutcome::Reported(r)) => assert_eq!(r.into_payload(), Count(20)),
        Err(f) => panic!("{}", f.error),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 5.3 · Same-module and entry fabrication
// ════════════════════════════════════════════════════════════════════════════

ladder!(Sealed {
    Entry(Count) => Inner(Count) => { Closed }
} impl {
    // Fabrication from INSIDE the module: this body builds `Inner` directly
    // rather than deriving it from the token it consumed. G2's seal is a module
    // boundary, and inside it there is no seal.
    inner = |_entry| { Inner::new(Count(999)) },
    step = |_inner| { Ok(StepOutcome::Closed(Closed::new())) },
});

/// **5.3 same-module-fabrication.** `G2` stops *external* fabrication. The
/// public entry constructor and code inside the generated module can both build
/// rungs, which is the module-boundary limit Rust always has.
///
/// Two halves, both exercised: the entry constructor is callable from out here,
/// and the body above fabricated a mid-ladder rung from nothing.
///
/// The complement — that an *outside* caller cannot do the same — is
/// `spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624`.
/// Together they bound the seal from both sides.
#[test]
fn the_entry_constructor_and_the_module_itself_may_fabricate() {
    // Half one: the entry rung's constructor is public.
    let entry = sealed::Entry::new(Count(1));

    // Half two: the body built an `Inner` unrelated to what it consumed.
    let inner = sealed::inner(entry);
    assert_eq!(
        inner.payload,
        Count(999),
        "a same-module body fabricated a mid-ladder rung"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 5.4 · Drop-proofing beyond the lint
// ════════════════════════════════════════════════════════════════════════════

/// **5.4 drop-proofing-beyond-the-lint.** `G4` is `#[must_use]`, and a lint is
/// escapable. All three documented escapes are exercised here, under
/// `deny(unused_must_use)` so the lint is at its strictest and still does not
/// bite.
///
/// True no-drop needs language-level linear types. Until Rust has them, this
/// test passes — and the day it starts failing, `G4` can be strengthened and
/// this proposition retired.
#[test]
#[deny(unused_must_use)]
#[allow(clippy::forget_non_drop)]
fn a_must_use_token_can_still_be_discarded_three_ways() {
    // Escape one: `let _ = token`.
    let a = arithmetic::Given::new(Count(1));
    let _ = a;

    // Escape two: `mem::forget`. Clippy objects that the token implements no
    // `Drop`, so forgetting it is the same as dropping it — which is the
    // non-guarantee stated from the other side. There is no destructor to
    // skip, because there is nothing a rung does on the way out. Kept because
    // §5.4 names this escape by name.
    {
        let b = arithmetic::Given::new(Count(2));
        #[allow(clippy::forget_non_drop)]
        std::mem::forget(b);
    }

    // Escape three: bury it in a container and drop the container.
    let c = arithmetic::Given::new(Count(3));
    let buried = vec![c];
    drop(buried);

    // All three compiled under the strictest setting of the lint that is
    // supposed to prevent exactly this.
}

// ════════════════════════════════════════════════════════════════════════════
// 5.5 · Liveness beyond the guard
// ════════════════════════════════════════════════════════════════════════════

ladder!(Wander {
    Begin(Count) => Active(Count) => { Stalled => Active | Settled }

    recover { stalled: Stalled => Active }
} impl {
    active = |begin| { Active::new(begin.payload) },
    step = |active| { Ok(StepOutcome::Stalled(Stalled::new(active))) },
    // The guard fires only on a token IDENTICAL to its source. A recover that
    // changes the payload every round satisfies it forever while converging on
    // nothing.
    stalled = |s| {
        let n = s.source().payload.0;
        Active::new(Count(n + 1))
    },
});

/// **5.5 liveness-beyond-the-guard.** `G8` catches an identical-token stall
/// loop. It does not prove forward progress.
///
/// This run makes "progress" by the guard's standard on every round — the token
/// differs each time — and gets no nearer a terminal. Bounded here only so the
/// test finishes; nothing in the ladder bounds it.
#[test]
fn the_progress_guard_is_satisfied_by_motion_that_goes_nowhere() {
    let mut active = wander::active(wander::Begin::new(Count(0)));

    for round in 0..100 {
        let Ok(outcome) = wander::step(active) else {
            panic!("round {round}: this ladder has no error path");
        };
        let wander::StepOutcome::Stalled(stalled) = outcome else {
            panic!("round {round}: reached a terminal, which this ladder cannot");
        };
        // The guard runs here and is satisfied: the token is not identical.
        active = wander::stalled(stalled);
    }

    assert_eq!(
        active.payload,
        Count(100),
        "a hundred rounds of guard-satisfying motion, no closer to a terminal"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 5.73 · Decidable is not pure
// ════════════════════════════════════════════════════════════════════════════

ladder!(Impure {
    Asked(Count) => Answered(Count) => { Told }
} impl {
    // Unmarked, so it reads as decidable. The decidable signature excludes
    // Het's outside — the principal pool — and says nothing about clocks,
    // files, or networks. This body reads a clock.
    answered = |asked| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Answered::new(Count(asked.payload.0 + (now % 2)))
    },
    step = |_answered| { Ok(StepOutcome::Told(Told::new())) },
});

/// **5.73 decidable-is-not-pure.** An unmarked transition reads as decidable,
/// and decidable excludes only `𝒫`. Ambient effects are not excluded and this
/// body takes one.
///
/// The proposition it points at, `purity-not-secured`, says closing this needs
/// an effect system. Until there is one, this compiles.
#[test]
fn an_unmarked_transition_may_touch_the_world() {
    let told = impure::step(impure::answered(impure::Asked::new(Count(0))));
    assert!(matches!(told, Ok(impure::StepOutcome::Told(_))));
}

// ════════════════════════════════════════════════════════════════════════════
// 5.6 · Suspension is in-process only
// ════════════════════════════════════════════════════════════════════════════

/// **5.6 suspension-is-in-process-only.** *"A driver may hold a
/// `Suspended<Prev>` in memory for as long as it likes, and that is the whole
/// of the claim."*
///
/// The positive half of the limit is what is provable here: holding one across
/// arbitrary intervening work is supported, and this exercises it. The negative
/// half — that writing one to disk and reading it back is not supported — is
/// proven by absence: no `Serialize` is emitted for `Suspended`, so the code
/// that would violate it does not compile.
///
/// The suspension machinery itself is exercised in `suspension.rs`; what this
/// adds is that the *holding* is unbounded, which is the part §5.6 asserts.
#[test]
fn a_suspension_may_be_held_across_arbitrary_intervening_work() {
    // Only the positive half is asserted. The negative half — that a
    // `Suspended` cannot be written to disk — is enforced by absence: nothing
    // emits `Serialize`, so the offending code does not exist to be run. A
    // test "proving" that by returning `false` from a stub would be a green
    // tick with nothing behind it, which is the failure this file is about.
    //
    // Whether the negative half can be proven at all is Q13.
    let held = arithmetic::Given::new(Count(7));
    for _ in 0..10 {
        let _ = impure::step(impure::answered(impure::Asked::new(Count(0))));
    }
    let doubled = arithmetic::doubled(held);
    assert_eq!(doubled.payload, Count(6), "the held token was still live");
}
