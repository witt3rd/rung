//! `#[judgmental]` without a role is refused.
//!
//! judgmental-declares-role: a judgmental operation declares the competence
//! role required to discharge it. Without one there is no `Qualified<R>` to
//! demand, so there is no judgmental signature to emit — and a marker that
//! emits the decidable signature would be a marker that guarantees nothing.
//!
//! The intended diagnostic is the macro's own `compile_error!`.

use rung::ladder;

struct SpecData;
struct LoopState;

ladder!(Demo {
    Spec(SpecData) => #[judgmental] Active(LoopState) => { Done }
} impl {
    active = |spec, _q| { Active::new(LoopState) },
    step   = |_active|  { Ok(StepOutcome::Done(Done::new())) },
});

fn main() {}
