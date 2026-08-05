//! The consumer's view, from across a crate boundary.
//!
//! Rust compiles `tests/` as a separate crate linking the library, so
//! everything here is a downstream consumer of `rung_fixture` — which is the
//! boundary `cross-crate-provenance` speaks about.
//!
//! Two things are pinned, and the pair is the proposition:
//!
//! - **the seal crosses.** A consumer cannot build a mid-ladder rung.
//! - **provenance does not.** A consumer cannot tell a token derived from a
//!   real order from one derived from an order the upstream crate invented.
//!
//! The second is the non-guarantee, and like the others in
//! `rung/tests/non_guarantees.rs` it is proven by **exercising the gap** — so
//! it fails if the gap ever closes, which would be news worth having.

use rung_fixture::{Receipt, from_an_invented_order, traversed, work};

// ════════════════════════════════════════════════════════════════════════════
// 1 · What crosses
// ════════════════════════════════════════════════════════════════════════════

/// The consumer can drive the ladder, because the entry constructor is public
/// and the transition functions are. This is the intended use, and it works.
#[test]
fn a_consumer_may_place_an_order_and_drive_it() {
    let placed = work::Placed::new(rung_fixture::Order::placed(7));
    let active = work::active(placed);
    assert_eq!(active.payload, Receipt { processed: 7 });

    match work::step(active) {
        Ok(work::StepOutcome::Settled(s)) => {
            assert_eq!(s.into_payload(), Receipt { processed: 7 })
        }
        Err(f) => panic!("{}", f.error),
    }
}

/// **The seal crosses the boundary.** A consumer cannot construct `Active`
/// directly — `Active::new` is private to the emitted module, and the module is
/// in another crate.
///
/// Pinned by a `trybuild` case with a committed `.stderr`, not by a
/// `compile_fail` doctest: rustdoc does not verify the error code, so such a
/// test cannot tell the refusal it was written for from a typo
/// (`no-guarantee-cites-a-compile-fail-doctest`).
#[test]
fn a_consumer_cannot_construct_a_mid_ladder_rung() {
    trybuild::TestCases::new().compile_fail("tests/ui/consumer_fabricates.rs");
}

// ════════════════════════════════════════════════════════════════════════════
// 2 · What does not
// ════════════════════════════════════════════════════════════════════════════

/// **`cross-crate-provenance`, exercised.**
///
/// One token came from an order a caller placed. The other came from an order
/// the upstream crate invented — `Order`'s `invented` flag is private, so only
/// that crate could set it, and the flag never reaches the receipt.
///
/// The two tokens are **equal**. Not similar, not hard to tell apart: equal,
/// by the only comparison a consumer has. There is no accessor to check, no
/// provenance field to read, nothing to assert against. The whole of a
/// consumer's knowledge is the payload, and the payload is identical.
///
/// If a sub-crate per ladder ever closes this (Q2), `from_an_invented_order`
/// stops being constructible and this test stops compiling — which is the
/// signal that the non-guarantee can be retired.
#[test]
fn a_consumer_cannot_tell_a_real_order_from_an_invented_one() {
    let honest = traversed(42);
    let invented = from_an_invented_order(42);

    assert_eq!(
        honest.payload, invented.payload,
        "the two tokens differ, so something downstream can distinguish them"
    );

    // And there is nothing else to compare. `Active` exposes `payload` and
    // `carry`; this ladder declares no carry, so the payload is the whole of
    // it. A consumer wanting to reject the invented one has no predicate to
    // write — which is what "trusted" means in the proposition.
    let _: &Receipt = &honest.payload;
    let _: &Receipt = &invented.payload;
}

/// Both tokens drive to a terminal identically, so the gap is not merely at the
/// moment of receipt — it persists through the rest of the run.
///
/// A consumer that wanted to quarantine suspect tokens could not, because it
/// cannot mark one: the two are the same value of the same type.
#[test]
fn an_invented_order_settles_exactly_as_a_real_one_does() {
    let mut outcomes = Vec::new();
    for token in [traversed(5), from_an_invented_order(5)] {
        match work::step(token) {
            Ok(work::StepOutcome::Settled(s)) => outcomes.push(s.into_payload()),
            Err(f) => panic!("{}", f.error),
        }
    }
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0], Receipt { processed: 5 });
}

/// The upstream crate **can** tell them apart, and that asymmetry is the
/// proposition's content.
///
/// `Order::is_invented` is private to `rung_fixture`. This test cannot call it,
/// and the fact that it cannot is the demonstration: the knowledge exists, it
/// is simply not exportable through a rung. Sealing this needs the types in a
/// sub-crate the declaring crate cannot reach either.
#[test]
fn the_knowledge_exists_upstream_and_cannot_cross() {
    // `rung_fixture::Order::is_invented` does not resolve here. Nothing in the
    // consumer's vocabulary refers to it, which is why this test asserts on
    // what it *can* see and says the rest in prose.
    let invented = from_an_invented_order(1);
    assert_eq!(invented.payload.processed, 1);
    // Anything stronger would require the accessor the boundary withholds.
}
