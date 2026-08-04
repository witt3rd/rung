//! An `#[authorial(Role)]` transition cannot be called without its pen.
//!
//! The mirror of `gate_missing_token.rs`. Authorship requires standing over the
//! subject (judgment-refuses-authorship-requires), and the only witness of
//! standing is an `Authorized` pen minted by `Pool::authorize`. A transition
//! that transforms therefore has an arity a decidable one does not.
//!
//! The intended diagnostic is **E0061** — "this function takes 2 arguments but
//! 1 argument was supplied". Anything else (a parse error, E0433, E0601) would
//! mean this file guards nothing.

use rung::ladder;

struct LoopState;

// An `#[authorial(R)]` transition admits its pen only over the container the
// subject sits in (SPEC.md G14), so the source rung's payload must name one.
// Supplied here so that the ONLY error in this file is the one it exists to pin.
struct SpecData;
impl rung::Situated for SpecData {
    fn container(&self) -> &str {
        "cabinet"
    }
}

#[derive(Clone, Copy)]
struct Curator;
impl rung::Role for Curator {
    const NAME: &'static str = "curator";
}

ladder!(Demo {
    Spec(SpecData) => #[authorial(Curator)] Active(LoopState) => { Done }
} impl {
    active = |_spec, _pen| { Active::new(LoopState) },
    step   = |_active|  { Ok(StepOutcome::Done(Done::new())) },
});

fn main() {
    let spec = demo::Spec::new(SpecData);
    let _active = demo::active(spec);
}
