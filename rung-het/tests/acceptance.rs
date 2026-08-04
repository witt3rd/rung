//! The acceptance test — the pass as a chain of principals.
//!
//! **This is the test the effort exists to pass.** Everything else in
//! `rung-het` is scaffolding until this is green.
//!
//! Written against `docs/heteronomy/formalism.md` §6.1 as ruled 2026-08-03.
//! The pass is not four operations with gates; it is four operations *and a
//! chain of principals*, each acting on what the previous one produced, each
//! constrained relative to **whose authorship it is acting on**:
//!
//! ```text
//!   operation   gate                    acts                        relative to
//!   ──────────────────────────────────────────────────────────────────────────
//!   audit       decidable | judgmental  nobody | judge              the object
//!   propose     AUTHORIAL  (N32a)       author with standing        the object
//!   dispose     judgmental              judge                       THE PROPOSAL
//!   enact       authorial               author with standing        the object
//! ```
//!
//! Two of those are corrections landed today, and each closes a real hole:
//!
//! - **N32a** — `propose` was `conditional`, which resolves to `judgmental`
//!   and dispatches under disjointness, i.e. to the *Opponent's* side. That
//!   made the Opponent play the Proponent's move. Answering a verdict is
//!   authorship; it needs standing, not disjointness.
//! - **N6d** — disjointness is measured against the **argument**, not the
//!   model. At `audit` they coincide. At `dispose` they do not: the argument
//!   is a Proposal, whose provenance is its author's. A judge who authored a
//!   proposal is disjoint from the *model* by construction, so under the old
//!   model-relative reading it passed the filter and **could rule on its own
//!   proposal**. That was a live P0 hole.
//!
//! # The domain
//!
//! Invented, so nothing reads as a special case of a real artifact. Two
//! containers, each governed by its own theory — the second exists so that
//! relocation writes into governed territory (N32h).

use rung_het::*;

// ─────────────────────────────────────────────────────────────────────────
// The domain
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct Specimen {
    pub id: &'static str,
    pub mounted: bool,
    pub locality: Option<&'static str>,
    pub author: &'static str,
}

#[derive(Clone, Debug)]
pub struct Cabinet {
    pub specimens: Vec<Specimen>,
    /// Declared and never read. Capacity is a **worth-law** — how many of the
    /// conforming items to keep — and N33 forbids a Het theory from declaring
    /// one. Present to mark where HetOpt lands, and to make its absence
    /// visible rather than merely unmentioned.
    pub capacity: usize,
    pub curator: &'static str,
}

#[derive(Clone, Debug)]
pub struct Fieldbook {
    pub notes: Vec<Specimen>,
    pub keeper: &'static str,
}

impl Provenanced for Cabinet {
    fn provenance(&self) -> Prov {
        let mut tags: Vec<&'static str> = self.specimens.iter().map(|s| s.author).collect();
        tags.push(self.curator);
        Prov::of(tags)
    }
}

impl Provenanced for Fieldbook {
    fn provenance(&self) -> Prov {
        Prov::of([self.keeper])
    }
}

#[derive(Clone, Copy)]
pub struct Taxonomist;
impl Role for Taxonomist {
    const NAME: &'static str = "taxonomist";
}

theory!(cabinet for Cabinet {
    decidable  all_mounted = |c: &Cabinet| c.specimens.iter().all(|s| s.mounted);
    judgmental all_in_scope: Taxonomist;
});

theory!(fieldbook for Fieldbook {
    decidable  all_located = |f: &Fieldbook| f.notes.iter().all(|n| n.locality.is_some());
    judgmental all_observed: Taxonomist;
});

// ─────────────────────────────────────────────────────────────────────────
// Principals
// ─────────────────────────────────────────────────────────────────────────

pub struct Person {
    pub id: &'static str,
    pub prov: &'static [&'static str],
    pub roles: &'static [&'static str],
    /// What this principal is steward of — the **authorial** condition.
    /// Judgment demands disjointness; authorship demands standing. Opposite
    /// conditions, one pool, selected by the gate (N10).
    pub stewards: &'static [&'static str],
}

impl Provenanced for Person {
    fn provenance(&self) -> Prov {
        Prov::of(self.prov.iter().copied())
    }
}

impl Principal for Person {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }
}

impl Steward for Person {
    fn has_standing(&self, over: &str) -> bool {
        self.stewards.contains(&over)
    }
}

fn specimen(id: &'static str, mounted: bool, locality: Option<&'static str>) -> Specimen {
    Specimen {
        id,
        mounted,
        locality,
        author: "collector",
    }
}

fn cabinet_of(specimens: Vec<Specimen>) -> Cabinet {
    Cabinet {
        specimens,
        capacity: 10,
        curator: "curator",
    }
}

/// The curator: steward of the cabinet, so it may author. Its provenance
/// overlaps the cabinet's, so it may **not** judge it. That is the point.
const CURATOR: Person = Person {
    id: "curator",
    prov: &["curator"],
    roles: &[],
    stewards: &["cabinet", "fieldbook"],
};

fn outside_pool() -> Pool<Person> {
    Pool::new(vec![
        // Refused at audit: authored the material under judgment.
        Person {
            id: "collector",
            prov: &["collector"],
            roles: &["taxonomist"],
            stewards: &[],
        },
        Person {
            id: "academy",
            prov: &["academy"],
            roles: &["taxonomist"],
            stewards: &[],
        },
    ])
}

// ─────────────────────────────────────────────────────────────────────────
// THE ACCEPTANCE TEST — the pass, end to end
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn the_pass_runs_end_to_end_as_a_chain_of_principals() {
    let mut cab = cabinet_of(vec![
        specimen("s1", true, Some("north ridge")),
        specimen("s2", false, Some("south fen")), // fails decidably
        specimen("s3", true, None),               // out of scope — belongs in the fieldbook
    ]);
    let mut book = Fieldbook {
        notes: vec![],
        keeper: "keeper",
    };
    let pool = outside_pool();

    // ── audit, decidable: no principal, no outside ──────────────────────
    let mounted = cabinet::all_mounted::holds(&cab);
    assert!(!mounted.verdict().is_conforming());
    assert!(!mounted.consulted_outside());

    // ── audit, judgmental: a judge disjoint from THE OBJECT ─────────────
    let q = pool
        .qualify::<Taxonomist>(&cab)
        .expect("a disjoint taxonomist qualifies");
    assert_eq!(
        q.principal_id(),
        "academy",
        "the collector authored the specimens and must be refused"
    );
    let scope = cabinet::all_in_scope::settle(
        &cab,
        q,
        Verdict::NonConforming {
            reason: "s3 is an observation, not a specimen".into(),
        },
    );
    assert!(scope.consulted_outside());

    // ── propose: AUTHORIAL (N32a) — standing, not disjointness ──────────
    let pen = pool
        .authorize(&CURATOR, "cabinet")
        .expect("the curator holds standing over the cabinet");
    let relocation = Proposal::remedy(&pen, "s3", Edit::Relocate { to: "fieldbook" });

    // N32b: the Proposal's provenance is its AUTHOR's, not the model's.
    assert!(
        relocation.provenance().overlaps(&Prov::of(["curator"])),
        "a Proposal carries its author's provenance"
    );

    // ── dispose: a judge disjoint from THE PROPOSAL (N6d) ───────────────
    let qd = pool
        .qualify_for::<Taxonomist>(&relocation)
        .expect("the academy did not author this proposal");
    let ruling = dispose(&relocation, qd, Disposition::Accept);
    assert!(ruling.is_terminal() && ruling.is_affirming());

    // ── enact: standing again, and the TARGET's law guards the write ────
    let ok = enact(&mut cab, &mut book, &ruling, &pen);
    assert!(
        matches!(ok, Err(EnactError::TargetRefused { .. })),
        "s3 has no locality; the fieldbook's own law must refuse it (N32h)"
    );
    assert_eq!(cab.specimens.len(), 3, "a refused write changes nothing");
    assert!(book.notes.is_empty());

    // Satisfy the target's law, and the same authorized edit lands.
    cab.specimens[2].locality = Some("east wood");
    enact(&mut cab, &mut book, &ruling, &pen).expect("the target now admits it");
    assert_eq!(cab.specimens.len(), 2);
    assert_eq!(book.notes.len(), 1);
    assert!(
        fieldbook::all_located::holds(&book)
            .verdict()
            .is_conforming()
    );
}

// ─────────────────────────────────────────────────────────────────────────
// N6d — the P0 hole, closed
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn a_judge_may_not_dispose_on_a_proposal_it_authored() {
    // The hole N6d closes. Under the old model-relative reading, a judge who
    // authored a proposal was disjoint from the MODEL by construction — so it
    // passed the filter and could rule on its own work. Measuring against the
    // ARGUMENT catches it.
    let cab = cabinet_of(vec![specimen("s1", false, None)]);

    // A principal that is disjoint from the cabinet AND authored the proposal.
    let insider = Person {
        id: "academy",
        prov: &["academy"],
        roles: &["taxonomist"],
        stewards: &["cabinet"],
    };
    let pool = Pool::new(vec![insider]);

    let pen = pool.authorize(&insider, "cabinet").unwrap();
    let p = Proposal::remedy(&pen, "s1", Edit::Amend { note: "mount it" });

    // Disjoint from the cabinet — would have passed the old check.
    assert!(
        pool.qualify::<Taxonomist>(&cab).is_ok(),
        "this principal IS disjoint from the model"
    );
    // Not disjoint from its own proposal — refused.
    assert!(
        matches!(
            pool.qualify_for::<Taxonomist>(&p),
            Err(QualifyError::NonIdentityViolated { .. })
        ),
        "N6d: disjointness is measured against the argument, and the argument \
         here is the proposal this principal authored"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// N32c — the contest path
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn an_author_may_dispute_a_verdict_without_first_authoring_a_remedy() {
    // Before N32c the only contest lived at dispose — downstream of propose.
    // An author who believed the audit simply wrong had to first author a
    // remedy for the diagnosis they disputed, to obtain a vehicle to dispute it.
    let cab = cabinet_of(vec![specimen("s1", true, None)]);
    let pool = outside_pool();
    let pen = pool.authorize(&CURATOR, "cabinet").unwrap();

    let d = Proposal::dispute(&pen, "s1", "the scope law does not reach this item");
    assert!(d.is_dispute());
    assert!(
        d.edit().is_none(),
        "a dispute proposes no edit — there is nothing to enact"
    );

    // A dispute is still judged. The author does not overturn a verdict by
    // asserting it.
    let q = pool.qualify_for::<Taxonomist>(&d).unwrap();
    let ruling = dispose(&d, q, Disposition::RejectDiagnosis);
    assert!(ruling.is_terminal());
    assert!(!ruling.is_affirming(), "nothing is enacted on a dispute");
}

// ─────────────────────────────────────────────────────────────────────────
// N32d/e/f — rejection returns to the author, carrying the reason
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn reject_remedy_is_non_terminal_and_the_reason_reaches_the_author() {
    // `accept-with-mod` retired: a judge amending is AUTHORING, and N32 says a
    // Disposition is a ruling, not a revision. What replaces it is a rejection
    // carrying a REASON — advisory prose, not an edit. That distinction is what
    // keeps the judge inside the judgmental gate (N32e).
    let cab = cabinet_of(vec![specimen("s2", false, Some("south fen"))]);
    let pool = outside_pool();
    let pen = pool.authorize(&CURATOR, "cabinet").unwrap();

    let first = Proposal::remedy(&pen, "s2", Edit::Remove);
    let q = pool.qualify_for::<Taxonomist>(&first).unwrap();
    let ruling = dispose(
        &first,
        q,
        Disposition::RejectRemedy {
            reason: "removal is disproportionate; mount it instead".into(),
        },
    );

    assert!(!ruling.is_terminal(), "the object re-enters the loop");
    assert!(!ruling.is_affirming());
    assert_eq!(
        ruling.reason(),
        Some("removal is disproportionate; mount it instead")
    );

    // N32f: the re-proposal carries the chain. Without it an author can cycle
    // forever on the same objection and nothing downstream could detect it.
    let second = first.reproposed(&pen, &ruling, Edit::Amend { note: "mount it" });
    assert_eq!(second.attempt(), 2);
    assert_eq!(
        second.prior_reasons(),
        vec!["removal is disproportionate; mount it instead"]
    );
    let _ = cab;
}

// ─────────────────────────────────────────────────────────────────────────
// The vocabularies, pinned against the gate boundary
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn the_disposition_vocabulary_is_exactly_the_five_that_survive_the_gate() {
    // accept-with-mod and reject-with-alternative are both absent, and both
    // for the same reason: a judge supplying content is authoring, and no
    // principal can be simultaneously provenance-disjoint (to judge) and
    // provenance-containing (to author) on one object.
    assert_eq!(
        Disposition::VARIANTS,
        &[
            ("accept", true, true),
            ("reject-diagnosis", true, false),
            ("reject-remedy", false, false),
            ("defer", false, false),
            ("raises-questions", false, false),
        ],
        "(name, terminal, affirming)"
    );
}

#[test]
fn het_places_no_bound_on_re_entry() {
    // N32g, pinned as a LIMIT rather than closed. If no acceptable remedy
    // exists, reject-remedy re-enters forever. Every honest answer — evict the
    // object, bound the attempts, accept non-conformance as declared debt — is
    // worth-shaped, and N33 forbids a Het theory a worth-law.
    //
    // This test exists so an implementation cannot quietly paper over the limit
    // by giving up after three tries: that would be a worth-law smuggled in
    // under another name. Het surfaces the re-entering object to its outside.
    assert_eq!(
        Disposition::REENTRY_BOUND,
        None,
        "a bound here would be HetOpt's, not Het's"
    );
}
