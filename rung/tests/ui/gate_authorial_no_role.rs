//! `#[authorial]` without a role is refused.
//!
//! authorial-declares-standing: an authorial operation declares a standing
//! predicate, and authorial-qualifying-set makes the qualifying set a
//! conjunction — `capable(p, role(o)) ∧ standing(p, M)`. Both conjuncts are
//! required, so a marker that names no role can express only half the filter.
//! Emitting a pen that witnessed standing alone would make the competence
//! filter decorative; emitting the decidable signature would make the marker
//! guarantee nothing. There is no third option, so this is a refusal.
//!
//! The intended diagnostic is the macro's own `compile_error!`.

use rung::ladder;

struct SpecData;
struct LoopState;

ladder!(Demo {
    Spec(SpecData) => #[authorial] Active(LoopState) => { Done }
} impl {
    active = |spec, _pen| { Active::new(LoopState) },
    step   = |_active|  { Ok(StepOutcome::Done(Done::new())) },
});

fn main() {}
