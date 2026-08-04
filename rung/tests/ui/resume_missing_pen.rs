//! A `resume` edge cannot be called without its pen.
//!
//! The declaration is well-formed here — the edge names `#[authorial(Curator)]`
//! and the emitted `fn` takes an `::rung::Authorized<'_, Curator>`. What this
//! file pins is that the pen is a *parameter*, so a caller that has evidence
//! the raised matter terminated and no standing over the subject has no term to
//! write.
//!
//! The intended diagnostic is **E0061** — "this function takes 3 arguments but
//! 2 arguments were supplied". Anything else would mean this file guards
//! nothing.

use rung::ladder;

struct Matter;
struct Finding;

impl rung::Provenanced for Matter {
    fn provenance(&self) -> rung::Prov {
        rung::Prov::of(["author"])
    }
}

impl rung::Provenanced for Finding {
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

#[derive(Clone, Copy)]
struct Curator;
impl rung::Role for Curator {
    const NAME: &'static str = "curator";
}

ladder!(Demo {
    Posed(Matter) => #[judgmental(Adjudicator)] Answered(Finding) => { Closed }
    resume { revive: #[authorial(Curator)] Suspended(Posed) => Posed }
} impl {
    answered = |_posed, _q| { Ok(Answered::new(Finding)) },
    step     = |_answered| { Ok(StepOutcome::Closed(Closed::new())) },
    revive   = |s| { s.token },
});

fn main() {
    let suspended = demo::Suspended {
        token: demo::Posed::new(Matter),
        raised: rung::Raised::new("q-13", "is_well_posed"),
    };
    let evidence = rung::Terminated::of(&suspended.raised, "resolved");
    let _posed = demo::revive(suspended, evidence);
}
