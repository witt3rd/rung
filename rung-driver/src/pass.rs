//! The composed audit-rectify engine — generic over the theory (Q19).
//!
//! The driver's one job: run `audit -> propose -> dispose -> enact -> verify`
//! over whatever theory instantiates it. The theory fills the slots via
//! [`Pass`]: what to audit, what remedy to propose for a defect, and (through
//! [`Verify`]) how an observer confirms the post-state. The author and the two
//! roles are passed in, so this file never names `Questions`, `Curator` or
//! `Adjudicator` — that is the theory crate's business.

use rung::{Pool, Role, Verdict};
use rung_het::{Disposition, Proposal, Verify, dispose, enact};
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
}

/// **Audit-only mode**: run the audit and report what is wrong, without
/// rectifying. The engine's `audit` face for any theory.
pub fn audit_run<W: Audit>(world: &W) -> Vec<Finding> {
    world.audit()
}

/// What a cycle produced.
pub enum CycleOutcome {
    /// Audit found nothing to rectify.
    Clean,
    /// The loop closed. `verified` is the observer's read-back of the enacted
    /// edit; the `dispatched` record is written from the ruling's seal.
    Rectified {
        verified: bool,
        record: DispatchedRecord,
    },
}

/// Run one composed audit-rectify cycle over a theory's world.
///
/// `world` implements [`Pass<E>`] for the edit type; `author` is the acting
/// author (with, `standing` over the territory); `ARole`/`JRole` name the
/// authorial and judgmental gates of this theory's pass; `pool` holds the
/// principals for the judge role.
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

    // propose — the author, with standing, writes a typed remedy
    let pen = pool
        .authorize::<ARole, _>(&author, standing)
        .expect("the author holds standing over the territory");
    let edit = world.remedy(&claim);
    let proposal = Proposal::remedy(&pen, &claim.subject, edit.clone());

    // dispose — a judge disjoint from the author; the ruling seals its judgment
    let judge = pool
        .qualify_for::<JRole>(&proposal)
        .expect("a judge disjoint from the author");
    let ruling = dispose(&proposal, judge, Disposition::Accept)
        .expect("the licence was minted against this proposal");

    // enact — the author applies the edit to the world
    let _ = enact(world, &ruling, &pen).expect("the world admits the edit");

    // verify — the observer reads the post-state back, not the author's word
    let verified = world.confirms(&edit, &claim.subject);

    // the ruling's sealed judgment -> `tier: dispatched` bookkeeping
    let record = DispatchedRecord::from_judgment(
        &claim.sentence,
        JRole::NAME,
        ruling.judgment(),
        "2026-08-06",
    );

    CycleOutcome::Rectified { verified, record }
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
}
