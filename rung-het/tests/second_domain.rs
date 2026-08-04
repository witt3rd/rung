//! A second domain, to prove edit-required-not-typed is real.
//!
//! The claim: Het declares that a remedy carries an **edit** (remedy-carries-an-edit) and that
//! `enact` applies one (enact-makes-an-endofunctor), and does **not** enumerate edits. If that is
//! true, a domain whose edits look nothing like the cabinet's must run through
//! the same library with no change to it.
//!
//! So: issue triage. Its edits are `Fix | WontFix | Duplicate | Reprioritize`.
//! Nothing moves between containers; nothing is relocated; there is no
//! write-guard, because nothing is written anywhere governed. `WontFix` closes
//! a ticket *as non-conforming* — an act with no analogue in the cabinet at all.
//!
//! If this compiles against an unchanged `rung-het`, the edit vocabulary really
//! is the theory's. If the library had to learn the word `WontFix`, it was
//! never generic — it was the cabinet's vocabulary wearing a generic name.

use rung_het::*;

// ─────────────────────────────────────────────────────────────────────────
// The domain
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub id: &'static str,
    pub labelled: bool,
    pub priority: u8,
    pub closed: Option<&'static str>,
    pub reporter: &'static str,
}

#[derive(Clone, Debug)]
pub struct Tracker {
    pub issues: Vec<Issue>,
    pub maintainer: &'static str,
}

impl Provenanced for Tracker {
    fn provenance(&self) -> Prov {
        let mut tags: Vec<&'static str> = self.issues.iter().map(|i| i.reporter).collect();
        tags.push(self.maintainer);
        Prov::of(tags)
    }
}

#[derive(Clone, Copy)]
pub struct Triager;
impl Role for Triager {
    const NAME: &'static str = "triager";
}

/// The **authorial** competence for this domain — `role(o)`.
///
/// Distinct from `Triager`, and required of an author in its own right: the
/// authorial qualifying set is a conjunction (authorial-qualifying-set), so
/// standing over the tracker settles only its right half.
#[derive(Clone, Copy)]
pub struct Maintainer;
impl Role for Maintainer {
    const NAME: &'static str = "maintainer";
}

/// **This theory's edits.** Not Het's, and not the cabinet's.
///
/// edit-required-not-typed. `WontFix` is the case that makes the point: it closes an issue
/// *while agreeing it is non-conforming*. Nothing in the cabinet's
/// `Amend | Remove | Relocate` can express that, and nothing in Het needs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriageEdit {
    Fix { commit: &'static str },
    WontFix { reason: &'static str },
    Duplicate { of: &'static str },
    Reprioritize { to: u8 },
}

theory!(triage for Tracker {
    decidable  all_labelled  = |t: &Tracker| t.issues.iter().all(|i| i.labelled);
    decidable  none_untriaged = |t: &Tracker| t.issues.iter().all(|i| i.priority > 0);
    judgmental all_actionable: Triager;
});

/// The domain applies its own edits (enact-generic-over-edit). The library cannot: it does not
/// know what an issue is, and has never heard of `WontFix`.
impl Applies<TriageEdit> for Tracker {
    fn territory(&self) -> &'static str {
        "tracker"
    }

    fn apply(&mut self, object: &str, edit: &TriageEdit) -> Result<(), EnactError> {
        let issue = self
            .issues
            .iter_mut()
            .find(|i| i.id == object)
            .ok_or_else(|| EnactError::ObjectNotFound {
                object: object.to_string(),
            })?;
        match edit {
            TriageEdit::Fix { commit } => issue.closed = Some(commit),
            TriageEdit::WontFix { reason } => issue.closed = Some(reason),
            TriageEdit::Duplicate { of } => issue.closed = Some(of),
            TriageEdit::Reprioritize { to } => issue.priority = *to,
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Principals
// ─────────────────────────────────────────────────────────────────────────

pub struct Dev {
    pub id: &'static str,
    pub prov: &'static [&'static str],
    pub roles: &'static [&'static str],
    pub stewards: &'static [&'static str],
}

impl Principal for Dev {
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
        Prov::of(self.prov.iter().copied())
    }

    /// The oracle. The verdict is the outside's, not the caller's.
    fn rule(&self, _matter: &str) -> Verdict {
        Verdict::Conforming
    }
}
impl Steward for Dev {
    fn has_standing(&self, over: &str) -> bool {
        self.stewards.contains(&over)
    }
}

const MAINTAINER: Dev = Dev {
    id: "maintainer",
    prov: &["maintainer"],
    roles: &["maintainer"],
    stewards: &["tracker"],
};

fn pool() -> Pool<Dev> {
    Pool::new(vec![
        // Refused at audit: reported the issues under judgment.
        Dev {
            id: "reporter",
            prov: &["reporter"],
            roles: &["triager"],
            stewards: &[],
        },
        Dev {
            id: "reviewer",
            prov: &["upstream"],
            roles: &["triager"],
            stewards: &[],
        },
    ])
}

fn tracker() -> Tracker {
    Tracker {
        issues: vec![
            Issue {
                id: "i1",
                labelled: true,
                priority: 2,
                closed: None,
                reporter: "reporter",
            },
            Issue {
                id: "i2",
                labelled: false,
                priority: 0,
                closed: None,
                reporter: "reporter",
            },
        ],
        maintainer: "maintainer",
    }
}

// ─────────────────────────────────────────────────────────────────────────
// The same pass, a different vocabulary
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn a_domain_with_entirely_different_edits_runs_the_same_pass() {
    let mut trk = tracker();
    let p = pool();

    // audit, decidable — no principal
    assert!(!triage::all_labelled::holds(&trk).verdict().is_conforming());

    // audit, judgmental — the reporter is refused; the reviewer qualifies
    let q = p.qualify::<Triager>(&trk).expect("reviewer is disjoint");
    assert_eq!(q.principal_id(), "reviewer");
    let (q, judgment) = p
        .consult::<Triager>(&trk, "all_actionable")
        .expect("reviewer is disjoint, and is asked");
    let _ = triage::all_actionable::settle(&trk, q, judgment)
        .expect("the licence and the judgment are the reviewer's");

    // propose — authorial, and the edit is THIS theory's
    let pen = p
        .authorize::<Maintainer, _>(&MAINTAINER, "tracker")
        .unwrap();
    let proposal = Proposal::remedy(
        &pen,
        "i2",
        TriageEdit::WontFix {
            reason: "out of scope for this project",
        },
    );

    // dispose — a judge disjoint from the PROPOSAL
    let qd = p.qualify_for::<Triager>(&proposal).unwrap();
    let ruling = dispose(&proposal, qd, Disposition::Accept)
        .expect("the licence was minted against this very argument");

    // enact — the domain applies its own edit
    let landed = enact(&mut trk, &ruling, &pen).expect("the maintainer holds standing");
    assert_eq!(landed.object(), "i2");
    assert_eq!(trk.issues[1].closed, Some("out of scope for this project"));
}

#[test]
fn wont_fix_closes_an_issue_that_remains_non_conforming() {
    // The case with no cabinet analogue, and the reason the edit vocabulary
    // cannot live in the library: the issue is closed AND still fails the
    // theory's own law. Het has no opinion about that — it governs who acted
    // and whether the result was admitted, never what the act meant.
    let mut trk = tracker();
    let p = pool();
    let pen = p
        .authorize::<Maintainer, _>(&MAINTAINER, "tracker")
        .unwrap();

    let proposal = Proposal::remedy(
        &pen,
        "i2",
        TriageEdit::WontFix {
            reason: "working as intended",
        },
    );
    let qd = p.qualify_for::<Triager>(&proposal).unwrap();
    let ruling = dispose(&proposal, qd, Disposition::Accept)
        .expect("the licence was minted against this very argument");
    let landed = enact(&mut trk, &ruling, &pen).unwrap();
    assert_eq!(landed.object(), "i2");

    assert!(trk.issues[1].closed.is_some(), "the issue is closed");
    assert!(
        !triage::all_labelled::holds(&trk).verdict().is_conforming(),
        "and it still fails the theory's law — closing is not conforming"
    );
}

#[test]
fn the_pass_is_indifferent_to_which_vocabulary_it_carries() {
    // Both domains' sentences are gate-marked identically. The pass sees a
    // sort, sentences, and an edit type it never inspects.
    assert_eq!(
        triage::SENTENCES,
        &[
            ("all_labelled", "decidable"),
            ("none_untriaged", "decidable"),
            ("all_actionable", "judgmental"),
        ]
    );
}

#[test]
fn p0_holds_here_too_without_the_library_knowing_the_domain() {
    // The reporter authored the issues and is refused, exactly as the collector
    // was refused in the cabinet. Nothing about this check is domain-specific.
    let trk = tracker();
    let only_reporter = Pool::new(vec![Dev {
        id: "reporter",
        prov: &["reporter"],
        roles: &["triager"],
        stewards: &[],
    }]);
    assert!(matches!(
        only_reporter.qualify::<Triager>(&trk),
        Err(QualifyError::NonIdentityViolated { .. })
    ));
}

#[test]
fn a_pen_for_one_territory_does_not_authorize_another() {
    // The authorial condition is standing over THIS territory (one-pool-two-filters). A pen is
    // not a general licence to write: an author with standing over the cabinet
    // may not enact on the tracker, however sound the ruling.
    //
    // Without this assertion the check in `enact` can be deleted and every
    // other test still passes — which is how a guarantee becomes decorative.
    const CURATOR_ELSEWHERE: Dev = Dev {
        id: "curator",
        prov: &["curator"],
        roles: &["maintainer"],
        stewards: &["cabinet"], // NOT the tracker
    };

    let mut trk = tracker();
    let p = Pool::new(vec![CURATOR_ELSEWHERE]);

    // A well-formed pen — for somewhere else.
    let wrong_pen = p
        .authorize::<Maintainer, _>(&CURATOR_ELSEWHERE, "cabinet")
        .expect("the curator does hold standing over the cabinet");

    // A ruling that affirms. Nothing about it is defective.
    let maintainer_pool = pool();
    let right_pen = maintainer_pool
        .authorize::<Maintainer, _>(&MAINTAINER, "tracker")
        .unwrap();
    let proposal = Proposal::remedy(&right_pen, "i2", TriageEdit::Reprioritize { to: 1 });
    let qd = maintainer_pool.qualify_for::<Triager>(&proposal).unwrap();
    let ruling = dispose(&proposal, qd, Disposition::Accept)
        .expect("the licence was minted against this very argument");

    match enact(&mut trk, &ruling, &wrong_pen) {
        Err(EnactError::NoStandingOverTarget { author, target }) => {
            assert_eq!(author, "curator");
            assert_eq!(target, "tracker");
        }
        other => panic!("a cabinet pen must not enact on the tracker; got {other:?}"),
    }
    assert_eq!(trk.issues[1].priority, 0, "a refused write changes nothing");

    // The right pen, same ruling, lands.
    let landed = enact(&mut trk, &ruling, &right_pen).expect("the maintainer holds standing");
    assert_eq!(landed.object(), "i2");
    assert_eq!(trk.issues[1].priority, 1);
}
