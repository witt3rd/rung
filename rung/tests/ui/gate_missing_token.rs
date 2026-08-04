//! A `#[judgmental(Role)]` transition cannot be called without its token.
//!
//! The intended diagnostic is **E0061** — "this function takes 2 arguments but
//! 1 argument was supplied". Anything else (a parse error, E0433, E0601) would
//! mean this file guards nothing.

use rung::ladder;

struct SpecData;
struct LoopState;

#[derive(Clone, Copy)]
struct Reviewer;
impl rung::Role for Reviewer {
    const NAME: &'static str = "reviewer";
}

ladder!(Demo {
    Spec(SpecData) => #[judgmental(Reviewer)] Active(LoopState) => { Done }
} impl {
    active = |_spec, _q| { Active::new(LoopState) },
    step   = |_active|  { Ok(StepOutcome::Done(Done::new())) },
});

fn main() {
    let spec = demo::Spec::new(SpecData);
    let _active = demo::active(spec);
}
