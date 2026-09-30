//! epsilon-reported-with-verdict — a `Weight` has no constructor outside the
//! rendering path.
//!
//! The only mint is `Rendering::weighed`, which validates the weight and binds
//! it to the verdict it weighs; the only way into a `Judgment` is
//! `Principal::judgment`, which reads it off the oracle's `Response`. So a body
//! cannot state a confidence of its own, and cannot move a weight it holds
//! onto a rendering it builds.
//!
//! The intended diagnostic is **E0451** (private fields), twice: a `Weight`
//! literal, and a `Rendering` literal re-using a weight another judge rendered.
//! There is no `Weight::new` either: no function in `rung` returns a `Weight`
//! by value, so a caller only ever sees one by reference, inside a rendering.

use rung::{Rendering, Verdict, VerdictPoint, Weight};

fn main() {
    // A literal: the fields are private.
    let _forged = Weight {
        point: VerdictPoint::probability(0.99).unwrap(),
        confidence: 0.99,
        model: "me".to_string(),
    };

    // Re-attaching a weight someone else rendered onto a verdict of my own:
    // `Rendering`'s fields are private too, and no constructor takes a `Weight`.
    let honest = Rendering::weighed(
        Verdict::NonConforming { reason: "no".into() },
        VerdictPoint::probability(0.02).unwrap(),
        0.97,
        "a-judge",
    )
    .unwrap();
    let weight: Weight = honest.weight().unwrap().clone();
    let _moved = Rendering {
        verdict: Verdict::Conforming,
        weight: Some(weight),
    };
}
