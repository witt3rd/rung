//! A judgmental dispatch that can be **suspended**, and resumed later.
//!
//! A principal that cannot answer now must be able to answer later. Het already
//! has the shape and it is not a new summand: `adequacy-defined` makes adequacy
//! *"a qualifying judge exists **and** returns a verdict"*, so a judge that
//! exists and has not answered is adequacy **undischarged**, and
//! `adequacy-failure-returns-residual` says that returns the residual — the
//! argument unconsumed, re-entering. The suspension is the `+ A` of
//! `judgmental-arrow-shape`.
//!
//! Three things are pinned here.
//!
//! **The deferral is opaque and is not a verdict.** `Principal::rule` returns
//! either a rendered verdict or a deferral carrying a reference to what was
//! raised. Het never interprets that reference — the identity of a raised
//! question belongs to the theory, not to Het (`pool-is-opaque`). And a
//! deferral cannot become a `Judgment`: the R2 seal stands, so nothing that was
//! not answered can be presented as an answer.
//!
//! **The transition has somewhere to hand the argument back.** A
//! `#[judgmental(R)]` forward transition returns
//! `Result<Next, Suspended<Prev>>` — the same `Result<_, Carrier<from>>` shape a
//! branching transition already has, with a carrier that holds the raised
//! reference instead of an error string.
//!
//! **Resumption is authorial, and unguarded.** Reviving a suspended run writes
//! a rung of the outer ladder, which `G2` seals against from outside the
//! module; so the resume edge is emitted *inside* it, gated on evidence the
//! raised matter terminated and on a pen held by a principal with standing over
//! the subject. No progress guard: a raised question may take any number of
//! rounds, and a guard on re-entry is the eviction rule
//! `guarded-reentry-is-eviction` forbids.

use rung::{
    Authorized, Consulted, Judgment, Pool, Principal, Prov, Provenanced, Qualified, QualifyError,
    Raised, Response, Role, Situated, Steward, Terminated, Verdict, ladder,
};

// ── roles ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Adjudicator;
impl Role for Adjudicator {
    const NAME: &'static str = "adjudicator";
}

/// The **authorial** role the resume edge requires. Not the judge's role, and
/// deliberately so: the principal that ruled on the raised matter is
/// provenance-disjoint from the subject, which is exactly what disqualifies it
/// from writing to the subject (judgment-refuses-authorship-requires).
#[derive(Clone, Copy)]
struct Curator;
impl Role for Curator {
    const NAME: &'static str = "curator";
}

// ── principals ──────────────────────────────────────────────────────────────

struct Person {
    id: &'static str,
    prov: Prov,
    roles: &'static [&'static str],
    stewards: &'static [&'static str],
    /// Whether this principal raises a matter instead of answering. The whole
    /// of the change on the principal's side: it is still capable, still
    /// disjoint, still in the pool — it has simply not answered yet.
    defers: bool,
}

impl Principal for Person {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }
    fn authored(&self) -> Prov {
        self.prov.clone()
    }
    fn rule(&self, matter: &str) -> Response {
        if self.defers {
            // `"q-13"` is the *theory's* name for what was raised. Het carries
            // it and never reads it (raised-reference-is-opaque).
            Response::Deferred(Raised::new("q-13", matter))
        } else {
            Response::Rendered(Verdict::Conforming)
        }
    }
}

impl Steward for Person {
    fn has_standing(&self, over: &str) -> bool {
        self.stewards.contains(&over)
    }
}

static CURATOR: Person = Person {
    id: "cora",
    prov: Prov::empty(),
    roles: &["curator"],
    stewards: &["inquiries"],
    defers: false,
};

fn answering_pool() -> Pool<Person> {
    Pool::new(vec![Person {
        id: "ada",
        prov: Prov::empty(),
        roles: &["adjudicator"],
        stewards: &[],
        defers: false,
    }])
}

fn deferring_pool() -> Pool<Person> {
    Pool::new(vec![Person {
        id: "dara",
        prov: Prov::empty(),
        roles: &["adjudicator"],
        stewards: &[],
        defers: true,
    }])
}

/// A principal that holds standing over `inquiries` but is the wrong steward
/// for nothing — it is the *right* one. Its mirror below is the wrong one.
fn curator_pool() -> Pool<Person> {
    Pool::new(vec![Person {
        id: CURATOR.id,
        prov: CURATOR.prov.clone(),
        roles: CURATOR.roles,
        stewards: CURATOR.stewards,
        defers: CURATOR.defers,
    }])
}

// ════════════════════════════════════════════════════════════════════════════
// 1 · The deferral — opaque, and not a verdict
// ════════════════════════════════════════════════════════════════════════════

/// The subject a question is asked about.
#[derive(Clone, PartialEq, Debug)]
struct Matter(&'static str);

impl Provenanced for Matter {
    fn provenance(&self) -> Prov {
        Prov::of([self.0])
    }
}

impl Situated for Matter {
    fn container(&self) -> &str {
        "inquiries"
    }
}

/// **The seal, on the deferral side.** `Principal::judgment` is the only mint
/// for a `Judgment`, and it calls `rule`. When `rule` defers there is no
/// verdict, so there is nothing to seal — and the sealed form must say so
/// rather than manufacture one.
///
/// Mutation: make the deferring branch of `Principal::judgment` build a
/// `Judgment` anyway (any verdict at all will do — that is the point) and this
/// test reddens.
#[test]
fn a_deferral_is_not_a_judgment() {
    let dara = Person {
        id: "dara",
        prov: Prov::empty(),
        roles: &["adjudicator"],
        stewards: &[],
        defers: true,
    };
    match dara.judgment("is_well_posed") {
        Consulted::Deferred(raised) => {
            assert_eq!(raised.reference(), "q-13");
            assert_eq!(raised.matter(), "is_well_posed");
        }
        Consulted::Rendered(j) => panic!(
            "a principal that did not answer produced a Judgment naming it as \
             the judge: {j:?}"
        ),
    }
}

/// `Pool::consult` propagates the deferral. No licence is minted, because a
/// `Qualified` carries the outside's answer and there is not one yet.
#[test]
fn the_pool_propagates_a_deferral_and_mints_no_licence() {
    let pool = deferring_pool();
    let subject = Matter("author");

    match pool.consult::<Adjudicator>(&subject, "is_well_posed") {
        Err(QualifyError::JudgeDeferred(raised)) => {
            // The reference, and only the reference. WHICH of the two
            // consultations `consult` makes raised it — the role-answer that
            // would have gone in the licence, or the sentence itself — is the
            // pool's business; either one leaves adequacy undischarged.
            assert_eq!(raised.reference(), "q-13");
        }
        Err(other) => panic!("a deferral was reported as a filter failure: {other}"),
        Ok(_) => panic!("a deferring principal minted a licence"),
    }

    // The same on the plain mint. `qualify_for` promises a licence carrying the
    // outside's answer, and a deferring judge has not given one.
    assert!(matches!(
        pool.qualify_for::<Adjudicator>(&subject),
        Err(QualifyError::JudgeDeferred(_))
    ));

    // And a pool whose member answers is unaffected: the deferral is a distinct
    // outcome, not a new way for qualification to fail.
    assert!(
        answering_pool()
            .consult::<Adjudicator>(&subject, "is_well_posed")
            .is_ok()
    );
}

/// The reference is the theory's, carried and never read.
///
/// Het has no predicate over it, no ordering on it, and no notion of what
/// counts as a well-formed one — `pool-is-opaque` says Het never inspects an
/// inhabitant of the pool, and the identity of what an inhabitant raised is on
/// the same side of that line.
#[test]
fn the_raised_reference_is_carried_and_never_interpreted() {
    let raised = Raised::new("¶ anything at all §", "is_well_posed");
    assert_eq!(raised.reference(), "¶ anything at all §");

    // Evidence is *derived from* the raised matter, so it cannot name a
    // reference nobody raised.
    let evidence = Terminated::of(&raised, "resolved");
    assert_eq!(evidence.reference(), raised.reference());
    assert_eq!(evidence.terminal(), "resolved");
    assert!(evidence.answers(&raised));
    assert!(!evidence.answers(&Raised::new("q-99", "something else")));
}

// ════════════════════════════════════════════════════════════════════════════
// 2 · The residual channel on a judgmental forward transition
// ════════════════════════════════════════════════════════════════════════════

/// The outcome of the arrow, when it is answered. Its provenance is the
/// judge's, structurally — G15's epilogue still runs on the `Ok` arm.
#[derive(Clone, PartialEq)]
struct Finding {
    judgment: Judgment,
}

impl Provenanced for Finding {
    fn provenance(&self) -> Prov {
        self.judgment.provenance()
    }
}

ladder!(Inquiry {
    Posed(Matter)
        => #[judgmental(Adjudicator)] Answered(Finding)
        => { Closed }
    resume { revive: #[authorial(Curator)] Suspended(Posed) => Posed }
} impl {
    // The body may answer, or it may hand the argument back with the reference
    // to what was raised. Nothing else changed about it.
    answered = |posed, q| {
        if posed.payload.0 == "unanswerable" {
            Err(Suspended { token: posed, raised: ::rung::Raised::new("q-13", "is_well_posed") })
        } else {
            Ok(Answered::new(Finding { judgment: q.into_judgment() }))
        }
    },
    step = |_answered| { Ok(StepOutcome::Closed(Closed::new())) },
    // Re-entry: the argument was never consumed, so the resume edge hands it
    // back. It is emitted INSIDE the module — that is what makes it a legal
    // route to a mid-ladder rung at all (G2).
    revive = |s| { s.token },
});

/// The `+ A`. The argument comes back unconsumed, with the reference.
#[test]
fn a_judgmental_forward_transition_returns_the_argument_unconsumed() {
    // The signature is the assertion: coercing the emitted `fn` to a pointer of
    // the exact expected type fails to compile if the residual channel is not
    // in the return type.
    let answered_fn: fn(
        inquiry::Posed,
        Qualified<Adjudicator>,
    ) -> Result<inquiry::Answered, inquiry::Suspended<inquiry::Posed>> = inquiry::answered;

    let pool = answering_pool();
    let subject = Matter("unanswerable");
    let licence = pool
        .qualify_for::<Adjudicator>(&subject)
        .expect("ada is disjoint from the author");

    match answered_fn(inquiry::Posed::new(subject.clone()), licence) {
        Err(suspended) => {
            assert_eq!(suspended.raised.reference(), "q-13");
            // Unconsumed: the very argument, still a live token.
            assert_eq!(suspended.token.payload, subject);
        }
        Ok(_) => panic!("the transition answered a matter it cannot answer"),
    }
}

/// The answered arm is untouched, epilogue and all.
#[test]
fn an_answered_dispatch_still_produces_the_next_rung() {
    let pool = answering_pool();
    let subject = Matter("author");
    let licence = pool.qualify_for::<Adjudicator>(&subject).unwrap();
    match inquiry::answered(inquiry::Posed::new(subject), licence) {
        Ok(answered) => assert_eq!(answered.payload.judgment.judge_id(), "ada"),
        Err(s) => panic!(
            "an answerable matter was suspended on {}",
            s.raised.reference()
        ),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 3 · Resumption is authorial
// ════════════════════════════════════════════════════════════════════════════

fn suspend(subject: &Matter) -> inquiry::Suspended<inquiry::Posed> {
    let pool = answering_pool();
    let licence = pool.qualify_for::<Adjudicator>(subject).unwrap();
    match inquiry::answered(inquiry::Posed::new(subject.clone()), licence) {
        Err(s) => s,
        Ok(_) => panic!("expected a suspension"),
    }
}

/// The whole round trip: suspend, hold the run in memory, resume with evidence
/// and a pen, drive to a terminal.
#[test]
fn a_suspension_resumes_through_the_authorial_edge() {
    // The resume edge's signature is itself the G2 assertion: no pen, no term.
    let revive_fn: fn(
        inquiry::Suspended<inquiry::Posed>,
        Terminated,
        Authorized<'_, Curator>,
    ) -> inquiry::Posed = inquiry::revive;

    let subject = Matter("unanswerable");
    let suspended = suspend(&subject);

    // A driver may hold it. Nothing about the run is running.
    let evidence = Terminated::of(&suspended.raised, "resolved");

    let curators = curator_pool();
    let pen = curators
        .authorize::<Curator, Person>(&CURATOR, "inquiries")
        .expect("cora stewards inquiries");

    let posed = revive_fn(suspended, evidence, pen);
    assert_eq!(posed.payload, subject);
}

/// **Re-entry is unguarded** (`no-bound-on-reentry`,
/// `guarded-reentry-is-eviction`). The same suspension is produced and resumed
/// twice, with a payload that does not change — a raised question may take any
/// number of rounds, and the resume edge is not allowed to have an opinion
/// about how many.
///
/// Mutation: inject `must_progress` on the resume edge and this test reddens on
/// the first round, because a re-entry that hands the argument back *is* the
/// argument.
#[test]
fn the_same_suspension_resumes_twice_with_no_progress_guard() {
    let subject = Matter("unanswerable");
    let curators = curator_pool();

    let mut posed = inquiry::Posed::new(subject.clone());
    for round in 0..2 {
        let pool = answering_pool();
        let licence = pool.qualify_for::<Adjudicator>(&posed.payload).unwrap();
        let suspended = match inquiry::answered(posed, licence) {
            Err(s) => s,
            Ok(_) => panic!("round {round}: expected a suspension"),
        };
        let evidence = Terminated::of(&suspended.raised, "resolved");
        let pen = curators
            .authorize::<Curator, Person>(&CURATOR, "inquiries")
            .unwrap();
        posed = inquiry::revive(suspended, evidence, pen);
        assert_eq!(posed.payload, subject, "round {round}");
    }
}

/// The standing check, injected and unskippable. The body never mentions the
/// pen — `revive = |s| { s.token }` — and a pen minted over another container
/// is refused anyway.
///
/// Mutation: delete the injected `must_hold_standing_over` from the resume
/// path and this test reddens.
#[test]
#[should_panic(expected = "authorship requires standing over")]
fn resume_refuses_a_pen_over_another_container() {
    let elsewhere = Person {
        id: "elias",
        prov: Prov::empty(),
        roles: &["curator"],
        stewards: &["archive"],
        defers: false,
    };
    let pool = Pool::new(vec![elsewhere]);
    let holder = Person {
        id: "elias",
        prov: Prov::empty(),
        roles: &["curator"],
        stewards: &["archive"],
        defers: false,
    };
    let pen = pool
        .authorize::<Curator, Person>(&holder, "archive")
        .expect("elias stewards the archive");

    let suspended = suspend(&Matter("unanswerable"));
    let evidence = Terminated::of(&suspended.raised, "resolved");
    let _ = inquiry::revive(suspended, evidence, pen);
}

/// The terminal check. Evidence that some *other* raised matter terminated
/// resumes nothing: what the outer arrow awaits is the terminal of the run it
/// raised.
#[test]
#[should_panic(expected = "does not answer the matter this run raised")]
fn resume_refuses_evidence_from_another_raised_matter() {
    let suspended = suspend(&Matter("unanswerable"));
    let unrelated = Terminated::of(&Raised::new("q-99", "something else"), "dissolved");

    let curators = curator_pool();
    let pen = curators
        .authorize::<Curator, Person>(&CURATOR, "inquiries")
        .unwrap();
    let _ = inquiry::revive(suspended, unrelated, pen);
}

// ════════════════════════════════════════════════════════════════════════════
// 4 · The refusals, with their diagnostics committed
// ════════════════════════════════════════════════════════════════════════════

/// A resume edge that names no authorial role is refused at expansion.
///
/// This is the `G2` pin. A resume path reachable without an `Authorized` pen
/// would be a route to a mid-ladder rung that no principal was authorized to
/// write, so the edge cannot be *declared* without naming the standing it
/// requires.
#[test]
fn a_resume_edge_without_an_authorial_marker_is_refused() {
    trybuild::TestCases::new().compile_fail("tests/ui/resume_without_a_pen.rs");
}

/// And the call site: the pen is a parameter, so omitting it is E0061.
#[test]
fn calling_resume_without_a_pen_is_e0061() {
    trybuild::TestCases::new().compile_fail("tests/ui/resume_missing_pen.rs");
}
