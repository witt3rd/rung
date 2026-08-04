//! A `resume` edge that names no authorial role is refused at expansion.
//!
//! Reviving a suspended run produces a rung of the outer ladder. `G2` seals
//! that construction against everything outside the module, so the resume edge
//! is emitted *inside* it — and an edge inside the seal that anyone may call is
//! the seal with a door in it. Resumption is therefore authorial
//! (resumption-is-authorial): it requires a principal holding standing over the
//! subject, the same shape as `enact`.
//!
//! So the edge cannot be *declared* without naming the standing it requires.
//! The intended diagnostic is the macro's own `compile_error!` — not E0061,
//! which would only mean a caller forgot an argument that could have been
//! omitted from the signature in the first place.

use rung::ladder;

struct Matter;
struct Report;

impl rung::Provenanced for Matter {
    fn provenance(&self) -> rung::Prov {
        rung::Prov::of(["author"])
    }
}

impl rung::Provenanced for Report {
    fn provenance(&self) -> rung::Prov {
        rung::Prov::empty()
    }
}

impl rung::Situated for Matter {
    fn container(&self) -> &str {
        "inquiries"
    }
}

#[derive(Clone, Copy)]
struct Adjudicator;
impl rung::Role for Adjudicator {
    const NAME: &'static str = "adjudicator";
}

ladder!(Demo {
    Posed(Matter) => #[judgmental(Adjudicator)] Answered(Report) => { Closed }
    resume { revive: Suspended(Posed) => Posed }
} impl {
    answered = |_posed, _q| { Ok(Answered::new(Report)) },
    step     = |_answered| { Ok(StepOutcome::Closed(Closed::new())) },
    revive   = |s| { s.token },
});

fn main() {}
