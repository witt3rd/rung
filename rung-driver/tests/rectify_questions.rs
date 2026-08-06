//! One audit-rectify cycle over rung's own questions, through the driver.
//!
//! This is the mechanism-first milestone. The seam it proves is the one that
//! was missing: `rung-driver` could build a pool and consult a judge, but it
//! could not reach the pass (`dispose` / `enact`). That join is what stands
//! between "the questions theory exists" and "the driver runs the loop".
//!
//! Everything here is deterministic — no network, no credential. The judge is
//! a local [`Oracle`] that answers `HOLDS`, so the cycle's *mechanism* is
//! exercised and asserted cold, in CI, on the real question files.
//!
//! Q14 is marked, not resolved. A real population's model principals carry the
//! empty-`authored` placeholder (pinned by `rung-driver/tests/oracle.rs`), so a
//! dispatched ruling today would qualify vacuously and mean nothing. This test
//! uses a judge whose provenance is **declared and disjoint** from the subject,
//! so the P0 check it runs through is real. The Q14 note is carried on the
//! record's tier, not papered over — see [`q14_marker`].
//!
//! The subject is real drift in rung's own questions. `affects_mirrors_inbound`
//! is a genuine, pinned defect (5 of 5 internal edges unacknowledged by their
//! source). The audit finds it, an author proposes mirroring one edge, a
//! disjoint judge accepts, and enact lands the edge — in the in-memory model
//! only, so the real files are untouched. That is the smallest real subject on
//! which the whole loop can be shown to bite.

use rung_driver::{Answer, Backing, Oracle, Population, population_pool};
use rung_het::{Disposition, Proposal, Verdict, Verify, dispose, enact};
use rung_std::questions::{Adjudicator, Curator, EdgeKind, QuestionEdit, Questions, Scheme};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// rung's own coordinates — the same `Scheme` the repository's own audit uses.
const RUNG: Scheme = Scheme {
    namespace: "rung-questions",
    root: "questions",
    id_prefix: "q",
};

fn questions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-driver sits in the workspace")
        .join("questions")
}

/// A deterministic `Oracle`: every consultation is answered `HOLDS`.
///
/// Local on purpose. A judge that cannot be disagreed with proves as much about
/// P0 and the pass as a real one can, and it runs with no network and no secret.
/// What is asserted here is that the *loop* closes — audit → propose → dispose
/// → enact — not that some particular model ruled one way.
struct Holding;

impl Oracle for Holding {
    fn ask(&self, _id: &str, _backing: &Backing, _matter: &str) -> Answer {
        Answer::holds()
    }
}

/// A population with the two roles the questions theory needs: an author
/// (curator, standing over the tree) and a judge (adjudicator, disjoint).
const POPULATION: &str = r#"
providers:
  - name: somewhere
    base_url: https://example.invalid/v1
    api_key_env: EXAMPLE_KEY

roles:
  - name: curator
    requires: [reasoning, file-editing]
  - name: adjudicator
    requires: [reasoning, structured-outputs]

principals:
  # The author. Holds standing over the whole questions tree, so it may
  # propose and enact. Its provenance is the namespace it files into, so it is
  # refused as a judge of what it authored (P0).
  - id: opus-author
    kind: agent
    capabilities: [reasoning, file-editing, curator]
    standing: [questions]
    authored: [rung-questions]
    backing: {via: outside}

  # The judge. Disjoint provenance, so it is admitted over the author's work.
  - id: external-judge
    kind: llm
    capabilities: [reasoning, structured-outputs, adjudicator]
    standing: []
    authored: [external-attestation]
    backing: {via: outside}
"#;

fn population() -> Population {
    Population::from_yaml(POPULATION).expect("the population parses")
}

/// The Q14 marker, carried on a dispatched record.
///
/// Until Q14 settles what provenance a model principal carries, a record whose
/// judge's provenance is the empty placeholder must say so rather than passing
/// as settled. In a real run this field would be the verdict against Q14's
/// ruling; here it states the gate is open.
fn q14_marker(judge: &str, provenance: &[String]) -> String {
    if provenance.is_empty() {
        format!(
            "{judge}: Q14 open — provenance is the empty placeholder; this ruling is provisional"
        )
    } else {
        format!("{judge}: provenance declared {provenance:?}; admissibility real under P0")
    }
}

#[test]
fn the_seam_runs_one_audit_rectify_cycle_over_rungs_own_questions() {
    let mut world = Questions::load(RUNG, &questions_dir());
    let pop = population();
    let pool = population_pool(&pop, "adjudicator", Arc::new(Holding));

    // ── 1 · AUDIT — the questions theory's own law, on the real files.
    //
    // `affects_mirrors_inbound` is violated: real internal edges are
    // unacknowledged by their source. The first drift tuple is the defect this
    // cycle exists to rectify.
    let settled = rung_std::questions::questions::affects_mirrors_inbound::holds(&world);
    assert!(
        !settled.consulted_outside(),
        "an outbound-edge check is decidable"
    );
    let Verdict::NonConforming { .. } = settled.verdict() else {
        panic!("the drift is real and pinned; a conforming audit means nothing to rectify")
    };

    let drift = world.outbound_drift();
    let (src, dependent, kind) = drift.first().expect("the pinned drift exists").clone();
    eprintln!(
        "  AUDIT   violates affects_mirrors_inbound: {src} --{kind}--> {dependent} unacknowledged"
    );

    // ── 2 · PROPOSE — authorial. The pen is minted over the tree; there is no
    //        term for proposing without standing (propose-is-authorial).
    let author = pop.by_id("opus-author").expect("declared").clone();
    let author_cfg = rung_driver::Configured::new(author.clone(), Arc::new(Holding));
    let pen = pool
        .authorize::<Curator, _>(&author_cfg, "questions")
        .expect("the author holds standing over the tree");
    let edge_kind = EdgeKind::parse(&kind).expect("the drift names a declared kind");
    let proposal = Proposal::remedy(
        &pen,
        &src,
        QuestionEdit::AddEdge {
            target: dependent.clone(),
            kind: edge_kind,
        },
    );

    // ── 3 · DISPOSE — judgmental. The licence is minted against THE PROPOSAL;
    //        its provenance is the author's, so the disjoint judge is admitted.
    let judge = pool
        .qualify_for::<Adjudicator>(&proposal)
        .expect("a judge disjoint from the proposal's author");
    let ruling = dispose(&proposal, judge, Disposition::Accept)
        .expect("the licence was minted against this proposal");
    assert!(ruling.is_affirming());
    eprintln!("  DISPOSE {} accepts mirroring the edge", ruling.judge());

    // ── 4 · ENACT — separate authorial arrow. The edit lands in the in-memory
    //        model; the real question files are untouched.
    let enacted = enact(&mut world, &ruling, &pen).expect("the tree admits the mirroring");
    assert_eq!(enacted.object(), src);

    // ── round-trip — the audit no longer flags that one edge.
    let remaining: Vec<_> = world
        .outbound_drift()
        .into_iter()
        .filter(|(s, d, _)| s == &src && d == &dependent)
        .collect();
    assert!(
        remaining.is_empty(),
        "the mirrored edge still drifts: {remaining:?}"
    );

    // ── 5 · VERIFY — the edit is observably in effect, read back by an
    //        observer, never taken from the author's report (`enact-verify`,
    //        the third failure point of enact). The same edit the loop
    //        enacted must be confirmed by the world's own state.
    let edit = QuestionEdit::AddEdge {
        target: dependent.clone(),
        kind: edge_kind,
    };
    assert!(
        world.confirms(&edit, &src),
        "after enact, the edge must be observably in effect — not merely claimed"
    );

    //    The claim-vs-state gap, exposed: the author's word is not the world's
    //    state. If the enacted edit had been a *different* one (say, the wrong
    //    target), verification refuses it even though a success was claimed.
    let impostor = QuestionEdit::AddEdge {
        target: "q99-does-not-exist".to_string(),
        kind: edge_kind,
    };
    assert!(
        !world.confirms(&impostor, &src),
        "a claimed-but-not-actually-applied edit must not verify — success is \
         attested by the state, not by the person who says so"
    );

    // The Q14 note, stated for the record. The judge's provenance is its
    // declared `authored`; in a dispatched record it would come out of the
    // sealed Judgment rather than a typed field. Either way, Q14's gate is
    // marked, not silently closed.
    let judge = pop.by_id("external-judge").expect("the judge is declared");
    eprintln!(
        "  RECORD  tier=dispatched, {}",
        q14_marker(judge.id.as_str(), &judge.authored)
    );
    eprintln!("  one audit-rectify cycle closed over rung's own questions");
}
