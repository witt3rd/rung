//! The acceptance test — the pass as a chain of principals.
//!
//! **This is the test the effort exists to pass.** Everything else in
//! `rung-het` is scaffolding until this is green.
//!
//! Written against `docs/rung-het-propositions.md` tower-is-a-fibration as
//! ruled 2026-08-03.
//! The pass is not four operations with gates; it is four operations *and a
//! chain of principals*, each acting on what the previous one produced, each
//! constrained relative to **whose authorship it is acting on**:
//!
//! ```text
//!   operation   gate                    acts                        relative to
//!   ──────────────────────────────────────────────────────────────────────────
//!   audit       decidable | judgmental  nobody | judge              the object
//!   propose     AUTHORIAL  (propose-is-authorial)       author with standing        the object
//!   dispose     judgmental              judge                       THE PROPOSAL
//!   enact       authorial               author with standing        the object
//! ```
//!
//! Two of those are corrections landed today, and each closes a real hole:
//!
//! - **propose-is-authorial** — `propose` was `conditional`, which resolves to `judgmental`
//!   and dispatches under disjointness, i.e. to the *Opponent's* side. That
//!   made the Opponent play the Proponent's move. Answering a verdict is
//!   authorship; it needs standing, not disjointness.
//! - **disjointness-against-argument** — disjointness is measured against the **argument**, not the
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
//! relocation writes into governed territory (target-runs-its-own-models).
//!
//! # The edits are the DOMAIN's (edit-required-not-typed)
//!
//! `CabinetEdit` is declared **here**, in the theory, not in `rung-het`. Het
//! requires that a remedy carry an edit (remedy-carries-an-edit) and that `enact` apply one
//! (enact-makes-an-endofunctor); it does not enumerate them. A GitHub-issue theory would declare
//! `Fix | WontFix | Duplicate`; this one declares amend, remove, relocate.
//! Those are the same operation to Het and different acts to their domains.
//!
//! Consequently `enact` is generic over this type (enact-generic-over-edit): the library cannot
//! apply an edit it did not name, so the domain supplies the application. Het
//! governs only *who may perform it* (one-pool-two-filters) and *whether the result is
//! admitted* (target-runs-its-own-models).

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
    /// conforming items to keep — and cut-at-valuation forbids a Het theory from declaring
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

/// The cabinet's edit vocabulary — **declared by this theory, not by Het**.
///
/// edit-required-not-typed: Het requires a remedy name an edit and does not say what edits are.
/// A triage theory would name `WontFix { reason }`; a portfolio theory would
/// name `Defund`. Neither is more or less Het-shaped than these three.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CabinetEdit {
    /// The specimen stays, changed.
    Amend { note: &'static str },
    /// The specimen leaves the cabinet.
    Remove,
    /// The specimen belongs elsewhere — and elsewhere is governed too, so the
    /// write runs the target's law (target-runs-its-own-models).
    Relocate { to: &'static str },
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
// The domain applies its OWN edits (enact-generic-over-edit)
// ─────────────────────────────────────────────────────────────────────────

/// Both containers together — what an edit of this domain acts on.
///
/// The library cannot apply `CabinetEdit`, because it does not know what a
/// cabinet or a fieldbook is. This impl is the theory saying what its own
/// edits do.
pub struct Collection {
    pub cabinet: Cabinet,
    pub fieldbook: Fieldbook,
}

impl Applies<CabinetEdit> for Collection {
    fn territory(&self) -> &'static str {
        "cabinet"
    }

    fn apply(&mut self, object: &'static str, edit: &CabinetEdit) -> Result<(), EnactError> {
        match edit {
            CabinetEdit::Amend { note } => {
                let s = self
                    .cabinet
                    .specimens
                    .iter_mut()
                    .find(|s| s.id == object)
                    .ok_or(EnactError::ObjectNotFound { object })?;
                if *note == "mount it" {
                    s.mounted = true;
                }
                Ok(())
            }
            CabinetEdit::Remove => {
                let i = self
                    .cabinet
                    .specimens
                    .iter()
                    .position(|s| s.id == object)
                    .ok_or(EnactError::ObjectNotFound { object })?;
                self.cabinet.specimens.remove(i);
                Ok(())
            }
            CabinetEdit::Relocate { to } => {
                let i = self
                    .cabinet
                    .specimens
                    .iter()
                    .position(|s| s.id == object)
                    .ok_or(EnactError::ObjectNotFound { object })?;

                // THE WRITE-GUARD (target-runs-its-own-models). The destination is governed too, so
                // its own law runs before the write lands — and may refuse an
                // edit the cabinet's judge already accepted.
                let item = &self.cabinet.specimens[i];
                if item.locality.is_none() {
                    return Err(EnactError::TargetRefused {
                        target: (*to).to_string(),
                        reason: format!(
                            "`{}` has no locality; the fieldbook requires one",
                            item.id
                        ),
                    });
                }

                let item = self.cabinet.specimens.remove(i);
                self.fieldbook.notes.push(item);
                Ok(())
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Principals
// ─────────────────────────────────────────────────────────────────────────

pub struct Person {
    pub id: &'static str,
    pub prov: &'static [&'static str],
    pub roles: &'static [&'static str],
    /// What this principal is steward of — the **authorial** condition.
    /// Judgment demands disjointness; authorship demands standing. Opposite
    /// conditions, one pool, selected by the gate (one-pool-two-filters).
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
    let mut world = Collection {
        cabinet: cabinet_of(vec![
            specimen("s1", true, Some("north ridge")),
            specimen("s2", false, Some("south fen")), // fails decidably
            specimen("s3", true, None),               // out of scope — belongs in the fieldbook
        ]),
        fieldbook: Fieldbook {
            notes: vec![],
            keeper: "keeper",
        },
    };
    let pool = outside_pool();

    // ── audit, decidable: no principal, no outside ──────────────────────
    let mounted = cabinet::all_mounted::holds(&world.cabinet);
    assert!(!mounted.verdict().is_conforming());
    assert!(!mounted.consulted_outside());

    // ── audit, judgmental: a judge disjoint from THE OBJECT ─────────────
    let q = pool
        .qualify::<Taxonomist>(&world.cabinet)
        .expect("a disjoint taxonomist qualifies");
    assert_eq!(
        q.principal_id(),
        "academy",
        "the collector authored the specimens and must be refused"
    );
    let scope = cabinet::all_in_scope::settle(
        &world.cabinet,
        q,
        Verdict::NonConforming {
            reason: "s3 is an observation, not a specimen".into(),
        },
    );
    assert!(scope.consulted_outside());

    // ── propose: AUTHORIAL (the-pass) — standing, not disjointness ───────────
    let pen = pool
        .authorize(&CURATOR, "cabinet")
        .expect("the curator holds standing over the cabinet");
    let relocation = Proposal::remedy(&pen, "s3", CabinetEdit::Relocate { to: "fieldbook" });

    // 7.24: the Proposal's provenance is its AUTHOR's, not the model's.
    assert!(
        relocation.provenance().overlaps(&Prov::of(["curator"])),
        "a Proposal carries its author's provenance"
    );

    // ── dispose: a judge disjoint from THE PROPOSAL (disjointness-against-argument) ──────────────
    let qd = pool
        .qualify_for::<Taxonomist>(&relocation)
        .expect("the academy did not author this proposal");
    let ruling = dispose(&relocation, qd, Disposition::Accept);
    assert!(ruling.is_terminal() && ruling.is_affirming());

    // ── enact: standing, and the DOMAIN applies its own edit (enact-generic-over-edit) ─────
    let refused = enact(&mut world, &ruling, &pen);
    assert!(
        matches!(refused, Err(EnactError::TargetRefused { .. })),
        "s3 has no locality; the fieldbook's own law must refuse it (target-runs-its-own-models)"
    );
    assert_eq!(
        world.cabinet.specimens.len(),
        3,
        "a refused write changes nothing"
    );
    assert!(world.fieldbook.notes.is_empty());

    // Satisfy the target's law, and the same authorized edit lands.
    world.cabinet.specimens[2].locality = Some("east wood");
    let landed = enact(&mut world, &ruling, &pen).expect("the target now admits it");
    assert_eq!(landed.moved(), "s3");
    assert_eq!(world.cabinet.specimens.len(), 2);
    assert_eq!(world.fieldbook.notes.len(), 1);
    assert!(
        fieldbook::all_located::holds(&world.fieldbook)
            .verdict()
            .is_conforming()
    );
}

// ─────────────────────────────────────────────────────────────────────────
// disjointness-against-argument — the P0 hole, closed
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn a_judge_may_not_dispose_on_a_proposal_it_authored() {
    // The hole disjointness-against-argument closes. Under the old model-relative reading, a judge who
    // authored a proposal was disjoint from the MODEL by construction — so it
    // passed the filter and could rule on its own work. Measuring against the
    // ARGUMENT catches it.
    let cab = cabinet_of(vec![specimen("s1", false, None)]);

    // A principal that is disjoint from the cabinet AND authored the proposal.
    const INSIDER: Person = Person {
        id: "academy",
        prov: &["academy"],
        roles: &["taxonomist"],
        stewards: &["cabinet"],
    };
    let pool = Pool::new(vec![INSIDER]);

    let pen = pool.authorize(&INSIDER, "cabinet").unwrap();
    let p = Proposal::remedy(&pen, "s1", CabinetEdit::Amend { note: "mount it" });

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
        "disjointness-against-argument: disjointness is measured against the argument, and the argument \
         here is the proposal this principal authored"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// proposal-vocabulary — the contest path
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn an_author_may_dispute_a_verdict_without_first_authoring_a_remedy() {
    // Before proposal-vocabulary the only contest lived at dispose — downstream of propose.
    // An author who believed the audit simply wrong had to first author a
    // remedy for the diagnosis they disputed, to obtain a vehicle to dispute it.
    let pool = outside_pool();
    let pen = pool.authorize(&CURATOR, "cabinet").unwrap();

    // The edit type must still be named: a dispute proposes no edit, but it is
    // a Proposal of THIS theory, and the theory has one edit vocabulary (edit-required-not-typed).
    let d: Proposal<CabinetEdit> =
        Proposal::dispute(&pen, "s1", "the scope law does not reach this item");
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
// no-amending-disposition/e/f — rejection returns to the author, carrying the reason
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn reject_remedy_is_non_terminal_and_the_reason_reaches_the_author() {
    // `accept-with-mod` retired: a judge amending is AUTHORING, and disposition-is-a-ruling says a
    // Disposition is a ruling, not a revision. What replaces it is a rejection
    // carrying a REASON — advisory prose, not an edit. That distinction is what
    // keeps the judge inside the judgmental gate (reason-is-not-an-edit).
    let pool = outside_pool();
    let pen = pool.authorize(&CURATOR, "cabinet").unwrap();

    let first = Proposal::remedy(&pen, "s2", CabinetEdit::Remove);
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

    // reproposal-carries-the-chain: the re-proposal carries the chain. Without it an author can cycle
    // forever on the same objection and nothing downstream could detect it.
    let second = first.reproposed(&pen, &ruling, CabinetEdit::Amend { note: "mount it" });
    assert_eq!(second.attempt(), 2);
    assert_eq!(
        second.prior_reasons(),
        vec!["removal is disproportionate; mount it instead"]
    );
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
    // no-bound-on-reentry, pinned as a LIMIT rather than closed. If no acceptable remedy
    // exists, reject-remedy re-enters forever. Every honest answer — evict the
    // object, bound the attempts, accept non-conformance as declared debt — is
    // worth-shaped, and cut-at-valuation forbids a Het theory a worth-law.
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
