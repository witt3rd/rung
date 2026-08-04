//! The audit–rectify pass, **declared as a `ladder!`**.
//!
//! `the-pass` is a chain of positions with a gate on every arrow. Until now the
//! chain was hand-rolled: `Proposal::remedy`, `dispose`, `enact` were free
//! functions and the loop between them lived in whatever driver called them.
//! Nothing typed the positions, so nothing could refuse a move made out of
//! order — and the crate that hosts `ladder!` was the last one still writing a
//! state machine by hand.
//!
//! Here the spine is a declaration:
//!
//! ```text
//! carry { subject_id: String, container: String }
//! Governed(Subject)
//!   => Audited(Verdict)
//!   => Proposing(Chain)                         // classification-only payload
//!   => #[authorial(Author)] Proposed(Proposal)  // propose-is-authorial
//!   => #[judgmental(Judge)] {
//!        Accept(Licence)
//!        | RejectDiagnosis(Why)
//!        | RejectRemedy    -> Proposing
//!        | Defer           -> Proposing
//!        | RaisesQuestions -> Audited
//!      }
//! ```
//!
//! Two things fall out of that shape and are pinned below rather than smoothed:
//!
//! 1. **`Proposing` carries classification only.** A continue arm's target rung
//!    is built inline by `step` — that is, by the **judge** (rung-props.md G10).
//!    If `Proposing`'s payload held proposal content, G10 would hand the judge
//!    an authoring position, which `disposition-is-a-ruling` and
//!    `no-amending-disposition` forbid. `Chain` is a concrete, non-generic type
//!    with no edit anywhere in it.
//!
//! 2. **Re-entry is a continue arm (`->`), never a recoverable verdict (`=>`).**
//!    A recoverable verdict makes the macro inject `must_progress`
//!    (rung-props.md G8), which panics on no progress — an eviction rule, which
//!    is exactly what `guarded-reentry-is-eviction` forbids.

use rung_het::*;

// ─────────────────────────────────────────────────────────────────────────
// The domain — a folio of drafts
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub id: &'static str,
    pub complete: bool,
    pub author: &'static str,
}

impl Provenanced for Draft {
    fn provenance(&self) -> Prov {
        Prov::of([self.author])
    }
}

impl Situated for Draft {
    fn container(&self) -> &str {
        "folio"
    }
}

/// The authorial competence — `role(o)`.
#[derive(Clone, Copy)]
pub struct Editor;
impl Role for Editor {
    const NAME: &'static str = "editor";
}

/// The judgmental competence — `role(φ)`.
#[derive(Clone, Copy)]
pub struct Reader;
impl Role for Reader {
    const NAME: &'static str = "reader";
}

/// The edits are the **theory's** (`edit-required-not-typed`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftEdit {
    Finish,
    Withdraw,
}

pub struct Folio {
    pub drafts: Vec<Draft>,
}

impl Applies<DraftEdit> for Folio {
    fn territory(&self) -> &'static str {
        "folio"
    }

    fn apply(&mut self, object: &str, edit: &DraftEdit) -> Result<(), EnactError> {
        let i = self
            .drafts
            .iter()
            .position(|d| d.id == object)
            .ok_or_else(|| EnactError::ObjectNotFound {
                object: object.to_string(),
            })?;
        match edit {
            DraftEdit::Finish => self.drafts[i].complete = true,
            DraftEdit::Withdraw => {
                self.drafts.remove(i);
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Principals
// ─────────────────────────────────────────────────────────────────────────

pub struct Person {
    pub id: &'static str,
    pub prov: &'static [&'static str],
    pub roles: &'static [&'static str],
    pub stewards: &'static [&'static str],
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
        Prov::of(self.prov.iter().copied())
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

/// Steward of the folio, so it may author. Provenance overlaps nothing the
/// reader has, so the reader may judge what it writes.
const EDITOR: Person = Person {
    id: "editor",
    prov: &["editor"],
    roles: &["editor"],
    stewards: &["folio"],
};

fn pool() -> Pool<Person> {
    Pool::new(vec![
        Person {
            id: "drafter",
            prov: &["drafter"],
            roles: &["reader"],
            stewards: &[],
        },
        Person {
            id: "reader",
            prov: &["reader"],
            roles: &["reader"],
            stewards: &[],
        },
        Person {
            id: "editor",
            prov: &["editor"],
            roles: &["editor"],
            stewards: &["folio"],
        },
    ])
}

fn folio() -> Folio {
    Folio {
        drafts: vec![Draft {
            id: "d1",
            complete: false,
            author: "drafter",
        }],
    }
}

fn carry_for(d: &Draft) -> pass::Carry {
    pass::Carry {
        subject_id: d.id.to_string(),
        container: "folio".to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// The pass, declared
// ─────────────────────────────────────────────────────────────────────────
//
// The spine, the gates and the Disposition vocabulary are **Het's** and come
// from the library. The three bodies below are the **theory's**: what its own
// law says, what its authors propose, and what its judges rule.

het_pass!(Pass {
    subject = Draft,
    edit = DraftEdit,
    author = Editor,
    judge = Reader,
} impl {
    audit = |d: &Draft| Verdict::conforming(d.complete, "the draft is not finished"),

    // The author reads the chain — attempt count and prior reasons — and
    // answers. It never sees a Disposition object, only what one left behind.
    propose = |chain: &Chain, _author: &str| {
        if chain.attempt() == 1 {
            Answer::Remedy(DraftEdit::Withdraw)
        } else {
            Answer::Remedy(DraftEdit::Finish)
        }
    },

    // The judge classifies. It has no constructor for a Proposal and no way to
    // amend one (`no-amending-disposition`); all it may add is prose.
    rule = |p: &Proposal<DraftEdit>, _judge: &str| match p.edit() {
        Some(DraftEdit::Withdraw) => Disposition::RejectRemedy {
            reason: "withdrawal is disproportionate; finish it instead".into(),
        },
        _ => Disposition::Accept,
    },
});

// ─────────────────────────────────────────────────────────────────────────
// 1 · the pass runs end to end as a ladder
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn the_pass_runs_end_to_end_as_a_ladder() {
    let mut world = folio();
    let pool = pool();

    // audit — unmarked, decidable: no principal, no outside.
    let subject = world.drafts[0].clone();
    let carry = carry_for(&subject);
    let audited = pass::audited(pass::Governed::new(subject, carry));
    assert!(
        !audited.payload.is_conforming(),
        "the draft is not finished, so the pass has something to answer"
    );

    let mut proposing = pass::proposing(audited);
    let mut rounds = 0usize;

    let landed = loop {
        rounds += 1;
        assert!(rounds < 10, "the loop did not converge");

        // propose — AUTHORIAL. There is no term for proposing without a pen.
        let pen = pool
            .authorize::<Editor, _>(&EDITOR, "folio")
            .expect("the editor holds standing over the folio");
        let proposed = pass::proposed(proposing, pen);

        // dispose — JUDGMENTAL, and the licence is minted against THE PROPOSAL.
        let judge = pool
            .qualify_for::<Reader>(&proposed.payload)
            .expect("a reader disjoint from the proposal's author");
        let outcome = match pass::step(proposed, judge) {
            Ok(o) => o,
            Err(f) => panic!("the step failed: {}", f.error),
        };
        match outcome {
            pass::StepOutcome::Accept(accepted) => {
                // enact — a SEPARATE authorial arrow, outside the branching
                // transition. `Accept` carries a licence, not a revised
                // subject: `Accept -> Governed` would have had the judge apply
                // the edit (`disposition-is-a-ruling`).
                let licence = accepted.into_payload();
                let pen = pool.authorize::<Editor, _>(&EDITOR, "folio").unwrap();
                break enact(&mut world, licence.ruling(), &pen).expect("the folio admits it");
            }
            pass::StepOutcome::RejectRemedy(next) | pass::StepOutcome::Defer(next) => {
                proposing = next
            }
            pass::StepOutcome::RaisesQuestions(_) => panic!("this judge asks nothing"),
            pass::StepOutcome::RejectDiagnosis(_) => panic!("this judge disputes no diagnosis"),
        }
    };

    assert_eq!(
        rounds, 2,
        "the first remedy was rejected, the second accepted"
    );
    assert_eq!(landed.object(), "d1");
    assert!(world.drafts[0].complete);
}

// ─────────────────────────────────────────────────────────────────────────
// 2 · propose cannot be called without an Authorized pen
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn calling_propose_without_a_pen_is_e0061() {
    trybuild::TestCases::new().compile_fail("tests/ui/pass_propose_without_a_pen.rs");
}

// ─────────────────────────────────────────────────────────────────────────
// 3 · dispose cannot be called without a Qualified token
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn calling_dispose_without_a_token_is_e0061() {
    trybuild::TestCases::new().compile_fail("tests/ui/pass_dispose_without_a_token.rs");
}

// ─────────────────────────────────────────────────────────────────────────
// 4 · the token `step` consumes is bound to the PROPOSAL, not the model
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "minted against a different argument")]
fn a_token_minted_against_the_model_is_refused_at_dispose() {
    // `disjointness-against-argument`. At `audit` the argument and the model
    // coincide; at `dispose` they do not — the argument is a Proposal, whose
    // provenance is its **author's**. A judge holding a licence minted against
    // the model has been measured against the wrong thing, and G13's injected
    // prologue refuses it whatever the body does with it.
    let world = folio();
    let pool = pool();

    let subject = world.drafts[0].clone();
    let carry = carry_for(&subject);
    let proposing = pass::proposing(pass::audited(pass::Governed::new(subject.clone(), carry)));
    let pen = pool.authorize::<Editor, _>(&EDITOR, "folio").unwrap();
    let proposed = pass::proposed(proposing, pen);

    // Honestly minted — against the MODEL. The filter ran and the reader passed
    // it, so nothing here is forged. It is simply bound to the wrong argument.
    let laundered = pool
        .qualify::<Reader>(&subject)
        .expect("a reader disjoint from the draft");
    let _ = pass::step(proposed, laundered);
}

// ─────────────────────────────────────────────────────────────────────────
// 5 · re-entry is unguarded — `no-bound-on-reentry`, `guarded-reentry-is-eviction`
// ─────────────────────────────────────────────────────────────────────────

// A second pass whose author never varies its answer and whose judge never
// accepts it. Under a recoverable verdict (`=>`) the macro would inject G8's
// `must_progress` here; under a continue arm (`->`) there is no guard to inject.
het_pass!(Stall {
    subject = Draft,
    edit = DraftEdit,
    author = Editor,
    judge = Reader,
} impl {
    audit = |d: &Draft| Verdict::conforming(d.complete, "the draft is not finished"),
    propose = |_chain: &Chain, _author: &str| Answer::Remedy(DraftEdit::Finish),
    rule = |_p: &Proposal<DraftEdit>, _judge: &str| Disposition::RejectRemedy {
        reason: "no.".into(),
    },
});

#[test]
fn reject_remedy_re_enters_with_no_progress_guard() {
    // `guarded-reentry-is-eviction`: re-entry is an **unguarded** return to the
    // authoring position. The author proposes the identical edit every round
    // and the judge rejects it with the identical reason every round; nothing
    // panics, nothing evicts, and the object is still in the loop after five
    // attempts.
    //
    // `no-bound-on-reentry`: there is no host-imposed bound. Adding one — even
    // "give up after three" — would be a worth-law smuggled in under another
    // name (`answers-are-worth-shaped`).
    let world = folio();
    let pool = pool();

    let subject = world.drafts[0].clone();
    let carry = stall::Carry {
        subject_id: subject.id.to_string(),
        container: "folio".to_string(),
    };
    let mut proposing = stall::proposing(stall::audited(stall::Governed::new(subject, carry)));

    for round in 1..=5usize {
        assert_eq!(
            proposing.payload.attempt(),
            round,
            "the chain grows even though the proposal never changes"
        );
        let pen = pool.authorize::<Editor, _>(&EDITOR, "folio").unwrap();
        let proposed = stall::proposed(proposing, pen);
        let judge = pool.qualify_for::<Reader>(&proposed.payload).unwrap();
        let outcome = match stall::step(proposed, judge) {
            Ok(o) => o,
            Err(f) => panic!("the step failed: {}", f.error),
        };
        match outcome {
            stall::StepOutcome::RejectRemedy(next) => proposing = next,
            _ => panic!("this judge only ever rejects the remedy"),
        }
    }

    assert_eq!(proposing.payload.attempt(), 6);
    assert_eq!(
        proposing.payload.prior_reasons().len(),
        5,
        "reproposal-carries-the-chain: every reason is carried forward"
    );
    assert_eq!(
        Disposition::REENTRY_BOUND,
        None,
        "a bound here would be HetOpt's, not Het's"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// 6 · `Proposing`'s payload cannot carry authored content
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn proposing_carries_classification_only() {
    // The constraint that falls out of G10. `step` builds the continue arm's
    // target rung **inline**, so whatever `Proposing` carries is authored by
    // the judge. `Chain` is therefore a concrete, non-generic type: it cannot
    // hold a `DraftEdit`, because it cannot hold any edit at all.
    let world = folio();
    let subject = world.drafts[0].clone();
    let carry = carry_for(&subject);
    let proposing = pass::proposing(pass::audited(pass::Governed::new(subject, carry)));

    // Type identity: the payload is exactly `rung_het::Chain`, the library's
    // classification record — not `Proposal<DraftEdit>`, not anything generic
    // in the theory's edit type.
    let chain: &Chain = &proposing.payload;

    assert_eq!(chain.subject_id(), "d1");
    assert_eq!(chain.container(), "folio");
    assert_eq!(chain.attempt(), 1);
    assert_eq!(chain.diagnosis(), Some("the draft is not finished"));
    assert!(chain.prior_dispositions().is_empty());
    assert!(chain.prior_reasons().is_empty());

    // Everything a `Chain` carries is prose or a count. A ruling that tried to
    // put an edit here would have no field to put it in — pinned as a
    // diagnostic, not as prose:
}

#[test]
fn a_chain_cannot_be_read_for_an_edit() {
    trybuild::TestCases::new().compile_fail("tests/ui/pass_chain_has_no_edit.rs");
}
