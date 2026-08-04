//! The demand a judgmental transition makes cannot be met by forging the token.
//!
//! `Qualified` has no public constructor: `Pool::qualify` is the only mint, and
//! it runs the competence filter and the non-identity filter first. So the
//! qualifying token is not a receipt someone remembered to produce — it is the
//! only way the term typechecks (non-identity-by-construction).
//!
//! The intended diagnostic is **E0451** — a private field in a struct literal.

use rung::ladder;
use std::marker::PhantomData;

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
    let forged: rung::Qualified<Reviewer> = rung::Qualified {
        _seal: (),
        _not_send: PhantomData,
        principal_id: String::from("nobody"),
        principal_prov: rung::Prov::empty(),
        // The binding half of the token (non-identity-by-construction) is
        // sealed too: forging `π(a)` would make every argument the right one.
        argument_prov: rung::Prov::empty(),
        _role: PhantomData,
    };
    let spec = demo::Spec::new(SpecData);
    let _active = demo::active(spec, forged);
}
