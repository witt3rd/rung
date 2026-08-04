//! `#[conditional(..)]` is refused, and the refusal names the open question.
//!
//! classifier-one-level-up: Het classifies a conditional gate per model, in the
//! theory one level up. `ladder!`'s checks run at expansion time against a
//! single declaration, which has no per-model level to consult. This is the
//! place Het's classification and rung's static checking do not meet, and it is
//! what remains of Q11's second blocker — so the message points at the question
//! rather than guessing an encoding.
//!
//! The intended diagnostic is the macro's own `compile_error!`.

use rung::ladder;

struct SpecData;
struct LoopState;

ladder!(Demo {
    Spec(SpecData) => #[conditional(IsSettledByContainment)] Active(LoopState) => { Done }
} impl {
    active = |spec, _q| { Active::new(LoopState) },
    step   = |_active|  { Ok(StepOutcome::Done(Done::new())) },
});

fn main() {}
