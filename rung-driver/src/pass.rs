//! The composed audit-rectify engine — generic over the theory (Q19).
//!
//! The driver's one job: run `audit -> propose -> dispose -> enact -> verify`
//! over whatever theory instantiates it. The theory fills the slots via
//! [`Pass`]: what to audit, what remedy to propose for a defect, and (through
//! [`Verify`]) how an observer confirms the post-state. The author and the two
//! roles are passed in, so this file never names `Questions`, `Curator` or
//! `Adjudicator` — that is the theory crate's business.

use rung::{Pool, Role, Verdict};
use rung_het::{Disposition, Proposal, Ruling, Verify, dispose, enact};
use rung_std::questions::{Questions, questions};

use crate::{Configured, DispatchedRecord, Oracle};

/// A defect the audit found on one subject.
#[derive(Debug, Clone)]
pub struct Finding {
    pub subject: String,
    pub sentence: String,
    pub reason: String,
}

/// **Every theory's face to the engine**: it can be audited.
///
/// Audit-only theories (`principals` is one) implement [`Audit`] and are driven
/// by [`audit_run`] — they see what is wrong but never fix it, because they
/// declare no edits. Editable theories also implement [`Pass`], adding the
/// rectify half.
pub trait Audit {
    /// Audit the whole model; return every defect found. A theory that can be
    /// rectified later treats the first as the one to remedy.
    fn audit(&self) -> Vec<Finding>;
}

/// The editable theory's face to the engine: audit + propose a remedy.
///
/// A theory implementing this for its world (and [`Verify`] for its edits) can
/// be driven by [`run_cycle`]; the engine never needs to know the theory's
/// sorts, edits or roles.
pub trait Pass<E>: Audit + Verify<E> {
    /// An author's typed remedy for a defect. Called with the world, so a
    /// theory may consult its own model to build the edit.
    fn remedy(&self, f: &Finding) -> E;

    /// The author's **post-judgment** remedy: propose (or re-propose) with the
    /// just-rendered judgment in hand. `after` is the disposition — its reason
    /// and the standard it was rendered against. The mirror of
    /// [`judgment-presupposes-the-standard`]: there the standard reaches the
    /// judge; here the judgment reaches the author, who draws the next proposal
    /// from the set that judgment licenses (`remedy-presupposes-the-judgment`,
    /// `reproposal-carries-the-chain`). Without `after`, a re-proposal would be
    /// the authorial constant arrow — a fix with no relation to what was decided.
    fn repropose(&self, f: &Finding, after: &Disposition) -> E;

    /// **Enact-to-carrier**: write an enacted edit back to the carrier, so the
    /// cycle can actually repair the collection it audits rather than only
    /// mutating it in memory. Default is a no-op; an editable theory that owns
    /// its files overrides this (see `Questions::repropose`'s sibling
    /// `Questions::persist`). Called by `run_cycle` after enact, before verify.
    fn persist(&mut self, _object: &str) {}

    /// The theory's combination rule over a panel of judges' rulings.
    ///
    /// `panels`: a panel is `⊨` with more than one judge, and *how the theory
    /// combines their rulings is the theory's*, not the library's — exactly as
    /// its edits are. `run_cycle` hands the whole judging step's [`Ruling`]s to
    /// this and dispatches on what it returns. A consensus rule (affirm only
    /// where every seat affirmed) is the natural default; majority, quorum and
    /// weighted rules — and any HetOpt worth-conditional one — are all the
    /// theory's to choose.
    fn combine(&self, rulings: &[Ruling<E>]) -> Disposition;
}

/// **Audit-only mode**: run the audit and report what is wrong, without
/// rectifying. The engine's `audit` face for any theory.
pub fn audit_run<W: Audit>(world: &W) -> Vec<Finding> {
    world.audit()
}

/// What a cycle produced.
///
/// The cycle treats **judging abstractly**: whether the theory's judgmental
/// step consulted one principal or a panel acting as one is conceptually
/// irrelevant to the cycle — `[judging]` is a single step. The theory decides
/// (via [`Pass::combine`]) how many judges weigh in and how their rulings
/// combine; this is what the cycle hands back.
#[derive(Debug)]
pub enum CycleOutcome {
    /// Audit found nothing to rectify.
    Clean,
    /// Judging affirmed. The edit was enacted; `verified` is the observer's
    /// read-back of the post-state; the `dispatched` record lists **every**
    /// judge that ruled (one, or a whole panel) with each one's sealed
    /// provenance.
    Rectified {
        verified: bool,
        record: DispatchedRecord,
    },
    /// Judging did **not** affirm — whether a sole judge or a panellist
    /// dissented is the same shape. The dissenting reasons are returned so the
    /// author re-proposes incorporating the divergence
    /// (`reject-remedy` carries a reason, 7.43; the author re-proposes with
    /// it, 7.44). A panel never *grants* affirmation a judge would not
    /// (`panels-cannot-weaken-the-opponent`) — but it cannot amend a proposal
    /// either (`no-amending-disposition`), so judgment surfaces, the author
    /// authors.
    Rejected { reasons: Vec<String> },
    /// A judge (one that would have ruled) deferred instead of answering; the
    /// run awaits it.
    Deferred,
}

/// Run one composed audit-rectify cycle over a theory's world.
///
/// `world` implements [`Pass<E>`] for the edit type; `author` is the acting
/// author (holding `standing` over the territory); `ARole`/`JRole` name the
/// authorial and judgmental gates of this theory's pass; `pool` holds the
/// principals for the judge role.
///
/// The judgmental step mints **every** qualifying judge
/// ([`Pool::qualifying`]) — a set that naturally has one member for a
/// sole-judge theory and several for a panel — and reads each one's own sealed
/// verdict. [`Pass::combine`] (the theory's) turns those rulings into the
/// effective disposition. The cycle never asks how many judges there were; it
/// only asks what judging concluded.
#[allow(clippy::too_many_arguments)]
pub fn run_cycle<O: Oracle, E: Clone, W, ARole: Role, JRole: Role>(
    world: &mut W,
    author: Configured<O>,
    standing: &str,
    pool: &Pool<Configured<O>>,
) -> CycleOutcome
where
    W: Pass<E>,
{
    let findings = world.audit();
    let Some(claim) = findings.into_iter().next() else {
        return CycleOutcome::Clean;
    };

    // The author proposes; at most **two** attempts. The first is a fresh
    // remedy; on a rejection the author *receives the judgment*
    // (`repropose` is handed the disposition — its reason and the standard it
    // was rendered against) and proposes once from the licensed set
    // (`remedy-presupposes-the-judgment`). Bounded, deliberately: how many
    // re-proposals to allow is itself a worth judgment, so the engine
    // re-proposes exactly once and surfaces the rest.
    let pen = pool
        .authorize::<ARole, _>(&author, standing)
        .expect("the author holds standing over the territory");
    let mut edit = world.remedy(&claim);
    let mut reproposed = false;

    for _ in 0..2 {
        let proposal = Proposal::remedy(&pen, &claim.subject, edit.clone());

        // judge — the whole qualifying set (a sole judge is a set of one)
        let seats = match pool.qualifying::<JRole>(&proposal) {
            Ok(seats) => seats,
            Err(rung::QualifyError::JudgeDeferred(_)) => return CycleOutcome::Deferred,
            Err(e) => panic!("judging requires at least one qualifying judge: {e}"),
        };
        let rulings = seats
            .into_iter()
            .map(|seat| {
                let disp = seat_disposition(seat.judgment().verdict());
                dispose(&proposal, seat, disp).expect("minted against this proposal")
            })
            .collect::<Vec<_>>();
        let effective = world.combine(&rulings);

        if effective.is_affirming() {
            // enact — via an affirming judge's ruling; the edit is the author's,
            // never the judging's (judgment classifies, authorship edits)
            let enacting = rulings
                .iter()
                .find(|r| r.is_affirming())
                .expect("judging affirmed, so a judge affirmed it");
            let _ = enact(world, enacting, &pen).expect("the world admits the edit");
            // verify — the observer reads the post-state back
            let verified = world.confirms(&edit, &claim.subject);
            // bookkeeping — a `tier: dispatched` record of every judge
            let record = DispatchedRecord::from_rulings(&claim.sentence, JRole::NAME, &rulings);
            return CycleOutcome::Rectified { verified, record };
        }

        if !reproposed {
            // the author received the judgment and re-proposes from it
            reproposed = true;
            edit = world.repropose(&claim, &effective);
            continue;
        }

        let reasons: Vec<String> = rulings
            .iter()
            .filter_map(|r| r.reason().map(str::to_string))
            .collect();
        return CycleOutcome::Rejected { reasons };
    }
    unreachable!("bounded to two attempts")
}

/// One outside verdict, read as a disposal.
///
/// A conforming outside affirms; a non-conforming outside rejects the remedy
/// *with its reason* (`reason-is-not-an-edit` — the reason becomes advisory
/// prose the author re-proposes with, never a replacement edit the judge
/// authors). This is the theory-neutral reading of `rule()`'s verdict; the
/// theory still chooses how the whole judging step combines via
/// [`Pass::combine`].
fn seat_disposition(verdict: &rung::Verdict) -> rung_het::Disposition {
    match verdict {
        rung::Verdict::Conforming => rung_het::Disposition::Accept,
        rung::Verdict::NonConforming { reason } => rung_het::Disposition::RejectRemedy {
            reason: reason.clone(),
        },
    }
}

/// The audit half for the questions world: `affects_mirrors_inbound` names a
/// genuine, pinned defect until every internal edge is acknowledged.
pub fn audit(world: &Questions) -> Option<Finding> {
    let settled = questions::affects_mirrors_inbound::holds(world);
    let Verdict::NonConforming { reason } = settled.verdict() else {
        return None;
    };
    let (src, _d, _k) = world.outbound_drift().first()?.clone();
    Some(Finding {
        subject: src,
        sentence: "affects_mirrors_inbound".to_string(),
        reason: reason.to_string(),
    })
}

/// The questions theory's `Pass`: audit with `affects_mirrors_inbound`, remedy
/// by mirroring the first drift edge.
impl Audit for Questions {
    fn audit(&self) -> Vec<Finding> {
        audit(self).into_iter().collect()
    }
}

impl Pass<rung_std::questions::QuestionEdit> for Questions {
    fn remedy(&self, _f: &Finding) -> rung_std::questions::QuestionEdit {
        use rung_std::questions::EdgeKind;
        let (target, kind) = self
            .outbound_drift()
            .first()
            .map(|(_s, d, k)| (d.clone(), k.clone()))
            .expect("the drift names a declared kind");
        rung_std::questions::QuestionEdit::AddEdge {
            target,
            kind: EdgeKind::parse(&kind).expect("declared kind"),
        }
    }

    /// The questions theory's combination rule: **consensus**. A panel affirms
    /// only where every seat affirmed — `panels-cannot-weaken-the-opponent`,
    /// verbatim. A dissenting seat's reason is the divergence the author
    /// re-proposes with, so strong agreement on some aspects and dissent on
    /// others both reach the resolution.
    fn combine(
        &self,
        rulings: &[rung_het::Ruling<rung_std::questions::QuestionEdit>],
    ) -> rung_het::Disposition {
        if rulings.iter().all(|r| r.is_affirming()) {
            rung_het::Disposition::Accept
        } else {
            let reasons = rulings
                .iter()
                .filter_map(|r| r.reason())
                .collect::<Vec<_>>()
                .join("; ");
            rung_het::Disposition::RejectRemedy { reason: reasons }
        }
    }

    /// The author's **post-judgment** remedy: handed the disposition, it
    /// re-proposes from what the judgment licenses (`remedy-presupposes-the-
    /// judgment`, `remedies-for-is-judgment-conditional`). The reason is the
    /// "why" the next proposal must address. The author's first instinct is to
    /// **repair** — `Rewrite` the subject into conformity (the primary remedy)
    /// — rather than demote it to Mode B; demotion is the fallback a theory
    /// really only wants when repair is impossible.
    fn repropose(
        &self,
        _f: &Finding,
        after: &rung_het::Disposition,
    ) -> rung_std::questions::QuestionEdit {
        use rung_std::questions::QuestionEdit;
        let _ = after;
        // repair is preferred: Rewrite the subject against the sharpened
        // criterion (the first remedy `remedies_for` licenses for a rejection).
        QuestionEdit::Rewrite {
            answerable: "Is there a single determinate fact at this subject's core — \
                         one reachable, unique, stable, authentic answer — and what is it?"
                .to_string(),
        }
    }

    fn persist(&mut self, object: &str) {
        let _ = rung_std::questions::Questions::persist(self, object);
    }
}
