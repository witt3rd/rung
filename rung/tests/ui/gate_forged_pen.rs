//! The demand an authorial transition makes cannot be met by forging the pen.
//!
//! `Authorized` has no public constructor: `Pool::authorize` is the only mint,
//! and it runs both conjuncts of the authorial qualifying set — capability, and
//! standing over the named container (authorial-qualifying-set). So the pen is
//! not a receipt someone remembered to produce; it is the only way the term
//! typechecks.
//!
//! The intended diagnostic is **E0451** — a private field in a struct literal.
//! Every field is named: an incomplete literal fails with E0063 whether or not
//! the fields are private, so it would keep failing with the seal removed and
//! would assert nothing (SPEC.md §6).

use rung::ladder;
use std::marker::PhantomData;

struct LoopState;

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
    let forged: rung::Authorized<'_, Curator> = rung::Authorized {
        _seal: (),
        _not_send: PhantomData,
        principal_id: String::from("nobody"),
        principal_prov: rung::Prov::empty(),
        // The container half of the pen is sealed too: forging `over` would
        // make every container the right one, and the standing predicate would
        // be enforced in name only.
        over: "cabinet",
        _role: PhantomData,
    };
    let spec = demo::Spec::new(SpecData);
    let _active = demo::active(spec, forged);
}
