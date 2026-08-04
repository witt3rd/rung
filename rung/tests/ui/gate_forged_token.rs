//! The demand a judgmental transition makes cannot be met by forging the token.
//!
//! `Qualified` has no public constructor: `Pool::qualify` is the only mint, and
//! it runs the competence filter and the non-identity filter first. So the
//! qualifying token is not a receipt someone remembered to produce — it is the
//! only way the term typechecks (non-identity-by-construction).
//!
//! The intended diagnostic is **E0451** — a private field in a struct literal.
//!
//! The forger below is a **real principal**, and holds a **real `Judgment`**
//! rendered by itself. That is the sharper version of this refusal after R2:
//! having something an outside actually said does not let you assemble the
//! licence that would let you spend it. The seal on the outcome and the seal on
//! the token are two seals, and holding one does not open the other.

use rung::{Principal, ladder};
use std::marker::PhantomData;

struct Nobody;

impl Principal for Nobody {
    fn capable(&self, _role_name: &str) -> bool {
        true
    }
    fn id(&self) -> &str {
        "nobody"
    }
    fn authored(&self) -> rung::Prov {
        rung::Prov::empty()
    }
    fn rule(&self, _matter: &str) -> rung::Verdict {
        rung::Verdict::Conforming
    }
}

struct SpecData;
struct LoopState;

// A `#[judgmental(R)]` transition admits its token only against the argument it
// was measured against (rung-props.md G13), so the source rung's payload must carry a
// provenance. Supplied here so that the ONLY error in this file is the one it
// exists to pin.
impl rung::Provenanced for SpecData {
    fn provenance(&self) -> rung::Prov {
        rung::Prov::of(["drafter"])
    }
}

// A judgmental transition's outcome is measured too (rung-props.md G15,
// π(f(a)) ⊆ π(p)), so the target payload must carry a provenance. Empty is
// admissible and is supplied here for the same reason as above: so that the
// ONLY error in this file is the one it exists to pin.
impl rung::Provenanced for LoopState {
    fn provenance(&self) -> rung::Prov {
        rung::Prov::empty()
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
        // Honestly obtained, and no help at all.
        judgment: Nobody.judgment("reviewer"),
        _role: PhantomData,
    };
    let spec = demo::Spec::new(SpecData);
    let _active = demo::active(spec, forged);
}
