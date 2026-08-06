//! The composed audit-rectify cycle, run by the generic driver.
//!
//! One cycle: audit -> propose -> dispose -> enact -> **verify** (the observer
//! reads the post-state back, not the author's word — `enact-verify`) — and,
//! now that a [`Ruling`] carries its sealed [`Judgment`] (Q12 made true of the
//! pass), a `tier: dispatched` record written from that seal, so the loop both
//! runs *and* writes.
//!
//! This is the questions instance's wiring: `run_cycle` is concrete for the
//! questions world so the sealed-token plumbing is typed and green. The
//! generic `Pass<E>` trait refinement is the next step toward a fully
//! theory-agnostic engine.

use std::sync::Arc;

use rung::{Pool, Verdict};
use rung_het::{Disposition, Proposal, Verify, dispose, enact};
use rung_std::questions::{Adjudicator, Curator, EdgeKind, QuestionEdit, Questions, questions};

use crate::{Configured, DispatchedRecord, Oracle, Population};

/// A defect the audit found on one subject.
#[derive(Debug, Clone)]
pub struct Finding {
    pub subject: String,
    pub sentence: String,
    pub reason: String,
}

/// The audit half, for the questions world: `affects_mirrors_inbound` names a
/// genuine, pinned defect until every internal edge is acknowledged.
pub fn audit(world: &Questions) -> Option<Finding> {
    let settled = questions::affects_mirrors_inbound::holds(world);
    let Verdict::NonConforming { reason } = settled.verdict() else {
        return None;
    };
    let (src, _dependent, _kind) = world.outbound_drift().first()?.clone();
    Some(Finding {
        subject: src,
        sentence: "affects_mirrors_inbound".to_string(),
        reason: reason.to_string(),
    })
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

/// Run one composed audit-rectify cycle over the questions world.
///
/// `pop` is the questions-capable population, `pool` the adjudicator pool, and
/// `oracle` the principal-facing ask. On a defect it drives
/// propose -> dispose -> enact -> verify and returns the `dispatched` record
/// the driver would write to `judgments/`.
pub fn run_cycle<O: Oracle>(
    world: &mut Questions,
    pop: &Population,
    pool: &Pool<Configured<O>>,
    oracle: Arc<O>,
) -> CycleOutcome {
    let Some(finding) = audit(world) else {
        return CycleOutcome::Clean;
    };
    let claim = finding;

    // propose — an author with standing over the tree
    let author = pop
        .by_id("opus-author")
        .expect("an author is declared")
        .clone();
    let author_cfg = Configured::new(author, oracle);
    let pen = pool
        .authorize::<Curator, _>(&author_cfg, "questions")
        .expect("the author holds standing over the tree");

    let (dependent, kind) = world
        .outbound_drift()
        .first()
        .map(|(_s, d, k)| (d.clone(), k.clone()))
        .expect("the pinned drift exists");
    let edge_kind = EdgeKind::parse(&kind).expect("the drift names a declared kind");
    let proposal = Proposal::remedy(
        &pen,
        &claim.subject,
        QuestionEdit::AddEdge {
            target: dependent.clone(),
            kind: edge_kind,
        },
    );

    // dispose — a judge disjoint from the proposal; the ruling seals its judgment
    let judge = pool
        .qualify_for::<Adjudicator>(&proposal)
        .expect("a judge disjoint from the proposal's author");
    let ruling = dispose(&proposal, judge, Disposition::Accept)
        .expect("the licence was minted against this proposal");

    // enact — the author applies the edit
    let _ = enact(world, &ruling, &pen).expect("the tree admits the mirroring");

    // verify — the observer reads the post-state back, not the author's word
    let edit = QuestionEdit::AddEdge {
        target: dependent,
        kind: edge_kind,
    };
    let verified = world.confirms(&edit, &claim.subject);

    // the ruling's sealed judgment -> `tier: dispatched` bookkeeping
    let record = DispatchedRecord::from_judgment(
        &claim.sentence,
        "adjudicator",
        ruling.judgment(),
        "2026-08-06",
    );

    CycleOutcome::Rectified { verified, record }
}
