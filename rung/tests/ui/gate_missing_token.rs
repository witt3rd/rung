//! A `#[judgmental(Role)]` transition cannot be called without its token.
//!
//! The intended diagnostic is **E0061** — "this function takes 2 arguments but
//! 1 argument was supplied". Anything else (a parse error, E0433, E0601) would
//! mean this file guards nothing.

use rung::ladder;

struct SpecData;
struct LoopState;

// A `#[judgmental(R)]` transition admits its token only against the argument it
// was measured against (SPEC.md G13), so the source rung's payload must carry a
// provenance. Supplied here so that the ONLY error in this file is the one it
// exists to pin.
impl rung::Provenanced for SpecData {
    fn provenance(&self) -> rung::Prov {
        rung::Prov::of(["drafter"])
    }
}

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
