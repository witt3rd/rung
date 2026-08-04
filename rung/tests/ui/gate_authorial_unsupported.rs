//! `#[authorial]` is refused rather than quietly accepted.
//!
//! The authorial gate is the mirror of the judgmental one — containment and
//! standing rather than disjointness (authorial-declares-standing) — and it
//! needs a third signature that this macro does not emit. Accepting the marker
//! and emitting the decidable signature would be the exact failure the gate
//! marker exists to prevent.
//!
//! The intended diagnostic is the macro's own `compile_error!`.

use rung::ladder;

struct SpecData;
struct LoopState;

ladder!(Demo {
    Spec(SpecData) => #[authorial] Active(LoopState) => { Done }
} impl {
    active = |spec, _q| { Active::new(LoopState) },
    step   = |_active|  { Ok(StepOutcome::Done(Done::new())) },
});

fn main() {}
