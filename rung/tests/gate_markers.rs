//! Gate markers on `ladder!` transitions — the first half of Q11.
//!
//! A `#[judgmental(Role)]` marker on a rung gives the transition that produces
//! it a second parameter, `Qualified<Role>`, which only `Pool::qualify` mints.
//! That makes the *signature* honest: a judgmental transition cannot be called
//! without an outside, and a decidable one has no parameter an outside could
//! enter through (decidable-cannot-consult-pool). It does **not** make the
//! arrow admissible — see the note at the bottom of this file.
//!
//! An `#[authorial(Role)]` marker gives it an `Authorized<'_, Role>` **pen**
//! instead. The two markers are not variants of one mechanism with different
//! token names: they are opposite conditions over one pool
//! (one-pool-two-filters). Judgment refuses the audited party; authorship
//! requires standing over it (judgment-refuses-authorship-requires). The
//! judgmental filter is provenance **disjointness**; the authorial filter is
//! capability **and standing** over the container the subject sits in
//! (authorial-qualifying-set, admissibility-subcategories). A principal that
//! qualifies as a judge of a subject is, on that evidence alone, *less* likely
//! to hold a pen over it, not more (provenance-overlap-is-the-point).
//!
//! The refusals are `trybuild` cases rather than `compile_fail` doctests
//! on purpose. rustdoc does not verify the error code on a `compile_fail`
//! block — a block annotated `compile_fail,E0999` passes — so a doctest cannot
//! distinguish "failed for the intended reason" from "failed because of a
//! typo". `trybuild` diffs the full stderr against a committed snapshot, so the
//! error code and the message text are both part of the assertion.

use rung::{
    AuthorizeError, Authorized, Judgment, Pool, Principal, Prov, Provenanced, Qualified, Response,
    Role, Situated, Steward, Verdict, ladder,
};

// ── a role, a principal, and a pool ─────────────────────────────────────────

#[derive(Clone, Copy)]
struct Reviewer;
impl Role for Reviewer {
    const NAME: &'static str = "reviewer";
}

#[derive(Clone, Copy)]
struct Judge;
impl Role for Judge {
    const NAME: &'static str = "judge";
}

struct Person {
    id: &'static str,
    prov: Prov,
    roles: &'static [&'static str],
    /// What this principal is steward of — the **authorial** condition. One
    /// pool, two filters (one-pool-two-filters): `roles`/`prov` feed the
    /// judgmental filter, `roles`/`stewards` the authorial one.
    stewards: &'static [&'static str],
}

impl Principal for Person {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }

    /// `authored` — the history this principal claims. `π(p)` is this
    /// **with `id()` added**, by the blanket `Provenanced` impl in `rung`:
    /// the provenance floor is not a value a principal gets to state.
    fn authored(&self) -> Prov {
        self.prov.clone()
    }

    /// The oracle. The verdict is the outside's, not the caller's.
    fn rule(&self, _matter: &str) -> Response {
        Response::Rendered(Verdict::Conforming)
    }
}

impl Steward for Person {
    fn has_standing(&self, over: &str) -> bool {
        self.stewards.contains(&over)
    }
}

// ── the ladder ──────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct SpecData(&'static str);

impl Provenanced for SpecData {
    fn provenance(&self) -> Prov {
        Prov::of([self.0])
    }
}

/// The judgmental arrow's outcome, and the shape R2 obliges it to have.
///
/// `rounds` is the body's to compute. `judgment` is not: it is the sealed
/// answer the outside gave, carried out of the licence by
/// `Qualified::into_judgment`, and `Provenanced` reads `π` off it. So the body
/// decides *what* comes back and cannot decide *whose* provenance it carries —
/// which is the "payload whose provenance is not freely chosen by the body"
/// that Q11 named as the thing that would close its load-bearing blocker.
///
/// Before R2 this struct declared `Prov::of(["drafter"])` — the provenance of
/// the very argument the arrow was called to judge — and passed every check the
/// workspace made.
#[derive(Clone, PartialEq)]
struct LoopState {
    judgment: Judgment,
    rounds: u32,
}

impl Provenanced for LoopState {
    fn provenance(&self) -> Prov {
        self.judgment.provenance()
    }
}

struct Report(u32);

// Both markable positions are marked: the forward transition `active`, and the
// branching transition `step` — `step` is the `dispose` position of Het's pass,
// where a judge rules on what the algebra produced.
ladder!(Review {
    Spec(SpecData)
        => #[judgmental(Reviewer)] Active(LoopState)
        => #[judgmental(Judge)] { Converged(Report) | Stalled => Active }
    recover { unstall: Stalled => Active }
} impl {
    active = |_spec, q| {
        assert_eq!(q.role_name(), "reviewer");
        // The residual channel is present and unused: this judge answers.
        Ok(Active::new(LoopState { judgment: q.into_judgment(), rounds: 0 }))
    },
    step = |active, q| {
        assert_eq!(q.role_name(), "judge");
        if active.payload.rounds >= 1 {
            Ok(StepOutcome::Converged(Converged::new(Report(active.payload.rounds))))
        } else {
            Ok(StepOutcome::Stalled(Stalled::new(active)))
        }
    },
    unstall = |stalled| {
        let active = stalled.into_source();
        let LoopState { judgment, rounds } = active.payload;
        Active::new(LoopState { judgment, rounds: rounds + 1 })
    },
});

// Two principals, because R2 makes the second necessary. `active`'s outcome
// now carries π(rita), so rita is no longer disjoint from what `step` is asked
// to judge — she authored it, in the only sense Het cares about, and P0 refuses
// her. That is the correct outcome and it used to be invisible: the old
// `LoopState` declared π = {drafter} whoever produced it, so rita could rule on
// her own ruling and no filter could see it.
fn pool() -> Pool<Person> {
    Pool::new(vec![
        Person {
            id: "rita",
            prov: Prov::of(["rita"]),
            roles: &["reviewer", "judge"],
            stewards: &[],
        },
        Person {
            id: "quinn",
            prov: Prov::of(["quinn"]),
            roles: &["judge"],
            stewards: &[],
        },
    ])
}

// ── 1. the positive case ────────────────────────────────────────────────────

#[test]
fn judgmental_transition_takes_a_qualified_token() {
    // The signature itself is the assertion. Coercing the generated `fn` item to
    // a `fn` pointer of the exact expected type fails to compile if the second
    // parameter is absent, extra, or of another type — so this line is what goes
    // red if the macro stops emitting the gate parameter.
    let active_fn: fn(
        review::Spec,
        Qualified<Reviewer>,
    ) -> Result<review::Active, review::Suspended<review::Spec>> = review::active;
    let step_fn: fn(
        review::Active,
        Qualified<Judge>,
    ) -> Result<review::StepOutcome, review::Failed<review::Active>> = review::step;

    let pool = pool();
    let spec = SpecData("drafter");

    // The token is minted, not written: `Pool::qualify` refuses a principal who
    // is not capable of the role, or who shares provenance with the material.
    let licence: Qualified<Reviewer> = pool.qualify(&spec).expect("rita qualifies as reviewer");
    assert_eq!(licence.principal_id(), "rita");

    let active = active_fn(review::Spec::new(spec), licence).expect("rita answers");
    assert_eq!(active.payload.rounds, 0);

    // Each dispatch re-runs the filter: the licence above was consumed by value.
    let ruling: Qualified<Judge> = pool
        .qualify(&active.payload)
        .expect("rita qualifies as judge");
    let active = match step_fn(active, ruling) {
        Ok(review::StepOutcome::Stalled(stalled)) => review::unstall(stalled),
        _ => panic!("expected a stall on the first round"),
    };

    let ruling: Qualified<Judge> = pool
        .qualify(&active.payload)
        .expect("rita qualifies as judge");
    match step_fn(active, ruling) {
        Ok(review::StepOutcome::Converged(done)) => assert_eq!(done.into_payload().0, 1),
        _ => panic!("expected convergence on the second round"),
    }
}

// ── 1b. the injected prologue (G13) — a body cannot skip the binding ────────
//
// The second half of Q11. G12 makes the *signature* honest; it does not make
// the *arrow* admissible, because a token minted against one argument could be
// spent on another. The macro therefore injects the binding check as a prologue
// — the same discipline it applies to `must_progress` for G8 — so the check
// cannot live in the body and the body cannot skip it.
//
// The ladder below is the adversarial case: its judgmental body **never
// mentions the token**. It binds it to `_q` and returns. Under G12 alone this
// arrow is "judgmental" and discharges nothing at all.

#[derive(Clone, PartialEq)]
struct Draft(&'static str);

impl Provenanced for Draft {
    fn provenance(&self) -> Prov {
        Prov::of([self.0])
    }
}

/// An outcome that claims no provenance at all.
///
/// Admissible, and worth being explicit about: `∅ ⊆ π(p)` so the epilogue
/// passes, and `∅ ∩ π(a) = ∅` so the proposition holds. A judgmental arrow may
/// return something authored by nobody; what it may not do is return something
/// authored by the party under judgment.
#[derive(Clone, PartialEq)]
struct Tally(u32);

impl Provenanced for Tally {
    fn provenance(&self) -> Prov {
        Prov::empty()
    }
}

ladder!(Blind {
    Manuscript(Draft)
        => #[judgmental(Reviewer)] Reviewed(Tally)
        => { Filed }
} impl {
    // No `q` in sight. Whatever this body proves, it is not that a qualified
    // outside was consulted about *this* manuscript.
    reviewed = |_manuscript, _q| { Ok(Reviewed::new(Tally(0))) },
    step     = |_reviewed| { Ok(StepOutcome::Filed(Filed::new())) },
});

#[test]
fn a_body_that_ignores_the_token_still_gets_the_binding_check() {
    // The licence is measured against the very manuscript it is spent on.
    let pool = pool();
    let manuscript = Draft("drafter");
    let licence: Qualified<Reviewer> = pool
        .qualify_for(&manuscript)
        .expect("rita is disjoint from the drafter");

    let reviewed =
        blind::reviewed(blind::Manuscript::new(manuscript), licence).expect("rita answers");
    assert_eq!(reviewed.payload.0, 0);
}

#[test]
#[should_panic(expected = "this qualifying token was minted against a different argument")]
fn the_injected_prologue_refuses_a_transferred_token_the_body_never_reads() {
    // Everything here is honestly obtained. `rita` really did pass both filters
    // against `someone-else`'s draft — the token is not forged, and the arrow's
    // signature is satisfied. What it was never measured against is the
    // manuscript it is about to license judgment on.
    let pool = pool();
    let elsewhere = Draft("someone-else");
    let transferred: Qualified<Reviewer> = pool
        .qualify_for(&elsewhere)
        .expect("rita is disjoint from someone-else");

    let manuscript = blind::Manuscript::new(Draft("drafter"));

    // The body would accept this without a murmur. The prologue does not.
    let _ = blind::reviewed(manuscript, transferred);
}

// ════════════════════════════════════════════════════════════════════════════
// The authorial gate (rung-props.md G14) — the OTHER filter over the SAME pool
// ════════════════════════════════════════════════════════════════════════════
//
// Everything below is the mirror of everything above, and the mirror is not a
// rename. A `Qualified<R>` says *"this principal did not author the argument."*
// An `Authorized<'_, R>` says *"this principal is capable of the role AND holds
// standing over the container the subject sits in"* (authorial-qualifying-set).
// Those are opposite conditions (judgment-refuses-authorship-requires): the
// author of a candidate IS the party under audit, so provenance overlap is what
// authorship needs and what judgment forbids (provenance-overlap-is-the-point).
//
// The subject payload below therefore carries BOTH coordinates — who wrote it
// (`Provenanced`, read by the judgmental filter) and where it sits (`Situated`,
// read by the authorial one) — so that a single value can be measured by both
// filters and the two answers can be shown to come apart.

/// The authorial competence — `role(o)` for an object of the cabinet.
#[derive(Clone, Copy)]
struct Curator;
impl Role for Curator {
    const NAME: &'static str = "curator";
}

#[derive(Clone, PartialEq)]
struct Sheet {
    /// The container this sheet sits in. What standing is held **over**.
    container: &'static str,
    /// Who wrote it. What disjointness is measured **against**.
    author: &'static str,
    revisions: u32,
}

impl Situated for Sheet {
    fn container(&self) -> &str {
        self.container
    }
}

impl Provenanced for Sheet {
    fn provenance(&self) -> Prov {
        Prov::of([self.author])
    }
}

/// Steward of the cabinet and of the annex, and the author of the sheets in
/// them. It may **not** judge them; that is the point, not the defect.
fn curator() -> Person {
    Person {
        id: "curator",
        prov: Prov::of(["curator"]),
        roles: &["curator"],
        stewards: &["cabinet", "annex"],
    }
}

/// Capable of the role, disjoint from the sheets — so it **qualifies as a
/// judge** of them. It stewards nothing.
fn stranger() -> Person {
    Person {
        id: "stranger",
        prov: Prov::of(["stranger"]),
        roles: &["curator"],
        stewards: &[],
    }
}

/// Steward of the cabinet, capable of nothing.
fn bystander() -> Person {
    Person {
        id: "bystander",
        prov: Prov::of(["bystander"]),
        roles: &[],
        stewards: &["cabinet"],
    }
}

fn cabinet_pool() -> Pool<Person> {
    Pool::new(vec![curator(), stranger(), bystander()])
}

ladder!(Revision {
    Filed(Sheet)
        => #[authorial(Curator)] Revised(Sheet)
        => { Published(u32) }
} impl {
    revised = |filed, pen| {
        assert_eq!(pen.principal_id(), "curator");
        assert_eq!(pen.role_name(), "curator");
        assert_eq!(pen.over(), "cabinet");
        let s = &filed.payload;
        Revised::new(Sheet { container: s.container, author: s.author, revisions: s.revisions + 1 })
    },
    step = |revised| { Ok(StepOutcome::Published(Published::new(revised.payload.revisions))) },
});

// ── 1. the positive case ────────────────────────────────────────────────────

#[test]
fn authorial_transition_takes_an_authorized_pen() {
    // The signature is the assertion, exactly as it is for the judgmental case:
    // coercing the generated `fn` item to a `fn` pointer of the expected type
    // fails to compile if the pen parameter is absent, extra, or of another
    // type — including if it is a `Qualified` rather than an `Authorized`.
    let revised_fn: fn(revision::Filed, Authorized<'_, Curator>) -> revision::Revised =
        revision::revised;

    let pool = cabinet_pool();
    let curator = curator();

    // The pen is minted, not written. Both conjuncts of the authorial
    // qualifying set run: capability, then standing (authorial-qualifying-set).
    let pen: Authorized<'_, Curator> = pool
        .authorize(&curator, "cabinet")
        .expect("the curator is capable and holds standing over the cabinet");
    assert_eq!(pen.principal_id(), "curator");
    assert_eq!(pen.over(), "cabinet");

    let sheet = Sheet {
        container: "cabinet",
        author: "curator",
        revisions: 0,
    };
    let revised = revised_fn(revision::Filed::new(sheet), pen);
    assert_eq!(revised.payload.revisions, 1);

    match revision::step(revised) {
        Ok(revision::StepOutcome::Published(done)) => assert_eq!(done.into_payload(), 1),
        _ => panic!("expected publication"),
    }
}

// ── 5. the injected prologue (G14) — a body cannot skip the standing check ──
//
// The adversarial ladder: its authorial body never mentions the pen. Under the
// signature rule alone this arrow is "authorial" and discharges nothing —
// exactly the hole G13 closes on the judgmental side. The pen carries the
// container it authorizes; the subject names the container it sits in; the
// macro compares them before the body runs.

ladder!(Careless {
    Draft(Sheet)
        => #[authorial(Curator)] Stamped(u32)
        => { Done }
} impl {
    // No `pen` in sight. Whatever this body proves, it is not that an author
    // with standing over *this* container did the writing.
    stamped = |_draft, _pen| { Stamped::new(1) },
    step    = |_stamped| { Ok(StepOutcome::Done(Done::new())) },
});

#[test]
fn a_body_that_ignores_the_pen_still_gets_the_standing_check() {
    let pool = cabinet_pool();
    let curator = curator();
    let pen = pool
        .authorize::<Curator, _>(&curator, "cabinet")
        .expect("standing over the cabinet");

    let draft = careless::Draft::new(Sheet {
        container: "cabinet",
        author: "curator",
        revisions: 0,
    });
    assert_eq!(careless::stamped(draft, pen).payload, 1);
}

#[test]
#[should_panic(expected = "this pen authorizes")]
fn the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads() {
    // Everything here is honestly obtained. The curator really does hold
    // standing over the annex — the pen is not forged, and the arrow's
    // signature is satisfied. What the curator was never authorized over is the
    // container this draft actually sits in.
    let pool = cabinet_pool();
    let curator = curator();
    let elsewhere = pool
        .authorize::<Curator, _>(&curator, "annex")
        .expect("the curator does steward the annex");

    let draft = careless::Draft::new(Sheet {
        container: "cabinet",
        author: "curator",
        revisions: 0,
    });

    // The body would accept this without a murmur. The prologue does not.
    let _ = careless::stamped(draft, elsewhere);
}

// ── 6. the asymmetry — this is not a renamed judgmental gate ────────────────

#[test]
fn standing_alone_is_not_a_pen_and_disjointness_never_becomes_one() {
    let pool = cabinet_pool();
    let sheet = Sheet {
        container: "cabinet",
        author: "curator",
        revisions: 0,
    };

    // ── (a) standing WITHOUT the declared role is refused ────────────────
    //
    // authorial-qualifying-set is a conjunction:
    //   P_auth(o, M) = { p : capable(p, role(o)) ∧ standing(p, M) }
    // The bystander satisfies the right conjunct and not the left. If
    // `authorize` checked standing only, this would mint a pen — which is the
    // exact shape in which the competence filter becomes decorative.
    let bystander = bystander();
    match pool.authorize::<Curator, _>(&bystander, "cabinet") {
        Err(AuthorizeError::NotCapable { principal, role }) => {
            assert_eq!(principal, "bystander");
            assert_eq!(role, "curator");
        }
        other => panic!(
            "standing without the declared role must not mint a pen; got {:?}",
            other.map(|p| p.principal_id().to_string())
        ),
    }

    // ── (b) qualifying as a JUDGE does not confer a pen ──────────────────
    //
    // The stranger is capable of the role and provenance-disjoint from the
    // sheet, so it passes the judgmental filter outright. Under a gate that was
    // the judgmental one with the token renamed, that would be the whole test
    // and the stranger would walk away with a pen.
    let stranger = stranger();
    let as_judge: Qualified<Curator> = pool
        .qualify_for(&sheet)
        .expect("someone in this pool is disjoint from the sheet");
    assert_eq!(
        as_judge.principal_id(),
        "stranger",
        "the curator authored the sheet, so the judgmental filter must skip it"
    );

    // The same principal, the same pool, the same role — and no pen. Standing
    // is not implied by disjointness; it is the opposite condition
    // (judgment-refuses-authorship-requires).
    match pool.authorize::<Curator, _>(&stranger, "cabinet") {
        Err(AuthorizeError::StandingIsJudgmental { principal, over }) => {
            assert_eq!(principal, "stranger");
            assert_eq!(over, "cabinet");
        }
        other => panic!(
            "disjointness from the subject must not confer standing over it; got {:?}",
            other.map(|p| p.principal_id().to_string())
        ),
    }

    // ── and the converse, which is the point (provenance-overlap-is-the-point)
    //
    // The curator holds the pen and is refused as a judge of the very same
    // sheet. One pool, two filters, opposite answers (one-pool-two-filters).
    assert!(
        pool.authorize::<Curator, _>(&curator(), "cabinet").is_ok(),
        "the curator stewards the cabinet"
    );
    let only_curator = Pool::new(vec![curator()]);
    assert!(
        only_curator.qualify_for::<Curator>(&sheet).is_err(),
        "the curator authored the sheet; the judgmental filter must refuse it"
    );
}

// ── 2–7. the refusals ───────────────────────────────────────────────────────
//
// One `trybuild::TestCases` per case, so a failure names the test that broke
// rather than a bundle. Each `.stderr` snapshot is the committed statement of
// *why* the case must fail; a case that starts failing for a different reason
// is a diff, not a pass.

#[test]
fn calling_a_judgmental_transition_without_a_token_is_e0061() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_missing_token.rs");
}

#[test]
fn a_qualified_token_cannot_be_constructed_outside_the_pool() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_forged_token.rs");
}

#[test]
fn judgmental_without_a_role_is_refused() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_judgmental_no_role.rs");
}

// ── 2. an authorial transition cannot be called without its pen ─────────────

#[test]
fn calling_an_authorial_transition_without_a_pen_is_e0061() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_authorial_missing_pen.rs");
}

// ── 3. the pen cannot be forged ─────────────────────────────────────────────

#[test]
fn an_authorized_pen_cannot_be_constructed_outside_the_pool() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_forged_pen.rs");
}

// ── 4. `#[authorial]` must name the role ────────────────────────────────────

#[test]
fn authorial_without_a_role_is_refused() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_authorial_no_role.rs");
}

#[test]
fn conditional_is_refused_and_names_the_open_question() {
    trybuild::TestCases::new().compile_fail("tests/ui/gate_conditional_unsupported.rs");
}

// ── 5. at most one marker (at-most-one-marker) ──────────────────────────────

#[test]
fn two_markers_on_one_transition_are_refused() {
    // Het's four gates are alternatives, not a set (four-gates). Two markers on
    // one transition ask for two second parameters and two prologues, and claim
    // the arrow is settled two ways at once. The macro has refused this since
    // markers landed; until this case existed nothing said so.
    trybuild::TestCases::new().compile_fail("tests/ui/gate_two_markers.rs");
}

// ── what these tests do and do not establish ────────────────────────────────
//
// The trybuild cases and the `fn`-pointer coercion are about the *signature*
// (G12): a judgmental transition cannot be called without a token, a decidable
// one has no parameter a token could enter through, and the token cannot be
// forged. `the_injected_prologue_refuses_a_transferred_token_the_body_never_reads`
// is about the *argument* (G13): the token records `π(a)` and the macro-injected
// prologue admits it only there, so a licence earned against one argument
// cannot be spent on another even by a body that never looks at it
// (non-identity-by-construction, disjointness-against-argument).
//
// The authorial cases add the second filter, in the same two layers:
// `authorial_transition_takes_an_authorized_pen` and the two `trybuild` cases
// are the *signature* (G14a — the pen cannot be omitted and cannot be forged);
// `the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads`
// is the *subject* (G14b — the pen is admitted only over the container the
// subject sits in). `standing_alone_is_not_a_pen_and_disjointness_never_becomes_one`
// is the one that establishes this is a second filter rather than a renamed
// first one: it shows the two filters returning opposite answers about the same
// principals over the same subject.
//
// What is still not established is that (G12 ∧ G13 ∧ G14) = gate-faithfulness.
// Two things remain outside: `#[conditional(..)]` is refused rather than
// implemented, so one of Het's four gates has no signature at all; the *value*
// a marked body returns is unconstrained, so admissibility of the returned
// value (`π(f(a)) ∩ π(a) = ∅` judgmentally, `π(f(a)) ⊆ π(p)` authorially) is a
// body property and inherits Q1's limit whole; and a decidable transition may
// still reach a clock or a socket, because the decidable signature excludes
// only Het's outside (purity-not-secured). The argument, and what would falsify
// it, is in Q11's note under docs/questions/open/.

// ── the gap, as a test rather than as a paragraph ───────────────────────────

/// **PARKED.** The judgmental arrow's *returned value* is unconstrained, and
/// this is the case that shows it.
///
/// `admissibility-subcategories` puts a judgmental operation in
/// `Kl_judg(𝒫) = { f : π(f(a)) ∩ π(a) = ∅ }`. Every check rung has runs on the
/// way **in**: G12 makes the signature demand a token, G13 binds that token to
/// `π(a)`, G14 does the authorial mirror. Nothing looks at what comes back.
///
/// So this passes every check rung makes, and is inadmissible: `active` is
/// `#[judgmental(Reviewer)]`, its argument's provenance is `{drafter}`, and the
/// `Active` it returns declares the very same provenance. The arrow launders
/// the material it was called to judge back out under a new rung name. The
/// engine cannot tell, because `Prov::contained_in` and `Prov::overlaps` exist
/// and no guarantee calls them on a return value.
///
/// That fixture is not a rigged one written for this test — it is the repo's
/// own gate-marker fixture, unchanged since markers landed. The condition has
/// been violated in-tree the whole time and nothing said so.
///
/// **Ignored, deliberately.** It is not a bug in the ladder above; it is
/// `returned-value-unconstrained` stated as an assertion instead of as prose,
/// so that the day rung constrains the return value someone can delete one
/// attribute and be told. Until then a green suite must not read as a claim
/// that gate-faithfulness holds.
#[test]
fn a_judgmental_arrow_may_not_return_the_provenance_it_judged() {
    let pool = pool();
    let spec = SpecData("drafter");

    let licence: Qualified<Reviewer> = pool
        .qualify_for(&spec)
        .expect("rita is disjoint from the drafter");

    let argument = spec.provenance();
    let active = review::active(review::Spec::new(spec), licence).expect("rita answers");

    assert!(
        !active.payload.provenance().overlaps(&argument),
        "admissibility-subcategories: a judgmental arrow inhabits \
         Kl_judg(P) = {{ f : π(f(a)) ∩ π(a) = ∅ }}. This one returned a value \
         carrying π(a) itself, and every gate rung has passed it"
    );
}

// ── the epilogue (R2) — a body cannot choose the outcome's provenance ───────
//
// `Launder` is `constant-arrow-hazard` written as a ladder: a judgmental
// transition whose body returns the argument it was handed, unchanged. Under
// G12 + G13 alone this arrow is well-marked, well-signed, and holds an honest
// licence bound to the very argument it is applied to — and it launders π(a)
// straight back out. It is the `settle(model, q, v)` hazard on the arrow
// surface: the value is drawn from `M`'s own carrier.
//
// The injected epilogue asserts `π(f(a)) ⊆ π(p)` on the way out, the mirror of
// the prologue's binding check on the way in. `{drafter}` is not contained in
// `{rita}`, so the arrow does not complete.

#[derive(Clone, PartialEq)]
struct Whisper(&'static str);

impl Provenanced for Whisper {
    fn provenance(&self) -> Prov {
        Prov::of([self.0])
    }
}

ladder!(Launder {
    Heard(Whisper)
        => #[judgmental(Reviewer)] Repeated(Whisper)
        => { Filed }
} impl {
    // c_j : a ↦ η(j), with j drawn from the argument itself.
    repeated = |heard, _q| { Ok(Repeated::new(heard.payload)) },
    step     = |_repeated| { Ok(StepOutcome::Filed(Filed::new())) },
});

#[test]
#[should_panic(expected = "π(f(a)) ⊄ π(p)")]
fn the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render() {
    let pool = pool();
    let heard = Whisper("drafter");
    let licence: Qualified<Reviewer> = pool
        .qualify_for(&heard)
        .expect("rita is disjoint from the drafter");

    // Every gate on the way in is satisfied. The way out is where this fails.
    let _ = launder::repeated(launder::Heard::new(heard), licence);
}

// ── the outward conditions that are still open ─────────────────────────────

/// **PARKED.** The *authorial* outward condition is unsecured, and so is the
/// outcome of a *branching* judgmental transition.
///
/// [G15](rung-props.md) closed the judgmental half on a **forward** transition:
/// `π(f(a)) ⊆ π(p)`, asserted by an injected epilogue, with disjointness
/// following. Two outward conditions are left, and this is the sharper one.
///
/// `admissibility-subcategories` states the authorial clause as
/// `π(f(a)) ⊆ π(p) ∧ standing(p, a)`. `G14` secures `standing` on the way in
/// and leaves the containment on the way out entirely to the body — the same
/// shape as `G13`'s gap, on the second gate. The arrow below is the case: the
/// curator holds an honest pen over the cabinet, the sheet sits in the cabinet,
/// and the revision it authors carries `someone-else`'s provenance. Nothing
/// refuses it.
///
/// The other residue has no separate case because it is a question rather than
/// a hole: a *branching* judgmental transition's recoverable and continue arms
/// carry the argument onward by design (`reproposal-carries-the-chain`,
/// `no-bound-on-reentry`), so which arms are outcomes in the sense of
/// `admissibility-subcategories` is unsettled, and an epilogue there would
/// refuse re-entry rather than laundering.
///
/// **Ignored, deliberately.** Unpark by deleting the attribute once an
/// authorial epilogue exists; it must then panic in the same place the
/// judgmental one does.
#[test]
#[ignore = "GAP, not a bug: G15 secures the JUDGMENTAL outward condition on a \
            FORWARD transition. The authorial one — π(f(a)) ⊆ π(p) from \
            admissibility-subcategories, the conjunct G14 left to the body — \
            has no epilogue, and neither does a branching judgmental \
            transition. This is what remains of Q11's blocker (1); see \
            docs/questions/open/q11-gate-faithfulness.md. Unpark by deleting \
            this attribute once an authorial epilogue exists."]
#[should_panic(expected = "\u{3c0}(f(a)) \u{2284} \u{3c0}(p)")]
fn an_authorial_arrow_may_not_return_a_provenance_its_author_does_not_hold() {
    let pool = cabinet_pool();
    let curator = curator();
    let pen: Authorized<'_, Curator> = pool
        .authorize(&curator, "cabinet")
        .expect("the curator holds standing over the cabinet");

    // Sits in the cabinet, so the standing prologue admits the pen. Written by
    // someone the curator is not.
    let sheet = Sheet {
        container: "cabinet",
        author: "someone-else",
        revisions: 0,
    };

    let revised = revision::revised(revision::Filed::new(sheet), pen);
    assert!(
        !revised
            .payload
            .provenance()
            .contained_in(&Prov::of(["curator"])),
        "the arrow returned a provenance its author does not hold, and no \
         guarantee looked"
    );
}

/// **PARKED.** `#[conditional(..)]` has no encoding, so an algebra with a
/// conditional operation cannot state gate-faithfulness here at all.
///
/// This is Q11's blocker (2), and it is not a matter of building more:
/// `conditional-partitions-fiber` partitions `Mod(Σ)` — a static property of
/// *which fiber a model sits in* — while rung's checks run at expansion time
/// against a declaration, and `classifier-one-level-up` requires the
/// classification be a sentence something can evaluate.
///
/// The cited file is the same declaration the refusal snapshot uses. Today the
/// macro rejects it with a `compile_error!` naming the open question; the day a
/// conditional marker has a signature, it compiles, and deleting the attribute
/// below reports that rather than leaving the reader to notice.
#[test]
#[ignore = "GAP: `#[conditional(..)]` is a parse-time refusal, not an \
            encoding. Gate-faithfulness quantifies over EVERY operation of an \
            algebra (rung-het-props.md#gate-faithful), so an algebra with a \
            conditional operation cannot state it here. This is Q11's blocker \
            (2); see docs/questions/open/q11-gate-faithfulness.md. Unpark by \
            deleting this attribute once the macro accepts the marker."]
fn a_conditional_marker_has_a_signature() {
    trybuild::TestCases::new().pass("tests/ui/gate_conditional_unsupported.rs");
}
