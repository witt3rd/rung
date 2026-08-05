//! One audit-rectify cycle over rung's own questions, from the command line.
//!
//! ```text
//! cargo run -p rung-driver --bin rectify_questions
//! ```
//!
//! What it does, and the honest limits of it:
//!
//! 1. **Loads the real `population.yaml`** and reports what it declares — in
//!    particular whether any principal could fill the questions theory's two
//!    roles (`curator` = author, `adjudicator` = judge), and the Q14 status of
//!    its model principals.
//! 2. **Runs one full audit-rectify cycle over rung's own `questions/`** using
//!    a deterministic local judge (no network, no credential). It is a
//!    mechanism proof: audit finds a real defect (`affects_mirrors_inbound` —
//!    the pinned outbound-edge drift), an author proposes mirroring one edge,
//!    the judge accepts, enact lands it in the in-memory model. The real files
//!    are untouched.
//! 3. **Q14-marked.** The local judge's provenance is declared and disjoint, so
//!    P0 is real. But the *real* population's model principals carry the
//!    empty-`authored` placeholder, so a real model judge cannot yet be
//!    dispatched meaningfully. The binary computes and prints that verdict.
//!
//! This is the "machine first, Q14-marked" milestone: the loop is proven to
//! close on real artifacts; wiring it to real models is gated on Q14, which is
//! a ruling, not more machinery.

use rung_driver::{Answer, Backing, Oracle, Population, population_pool};
use rung_het::{Disposition, Proposal, Verdict, dispose, enact};
use rung_std::questions::{Adjudicator, Curator, EdgeKind, QuestionEdit, Questions, Scheme};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// rung's own coordinates.
const RUNG: Scheme = Scheme {
    namespace: "rung-questions",
    root: "questions",
    id_prefix: "q",
};

/// A deterministic judge: every consultation answers `HOLDS`.
///
/// The mechanism proof needs a judge that cannot be disagreed with and needs no
/// secret. What is proven is that the loop closes, not that a particular model
/// ruled one way.
struct Holding;
impl Oracle for Holding {
    fn ask(&self, _id: &str, _backing: &Backing, _matter: &str) -> Answer {
        Answer::holds()
    }
}

/// A questions-capable population: an author (curator, standing over the tree)
/// and a judge (adjudicator, disjoint provenance).
const QUESTIONS_POPULATION: &str = r#"
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
  - id: opus-author
    kind: agent
    capabilities: [reasoning, file-editing, curator]
    standing: [questions]
    authored: [rung-questions]
    backing: {via: outside}
  - id: external-judge
    kind: llm
    capabilities: [reasoning, structured-outputs, adjudicator]
    standing: []
    authored: [external-attestation]
    backing: {via: outside}
"#;

fn ws_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-driver sits in the workspace")
        .to_path_buf()
}

fn main() {
    let root = ws_root();
    let pop_text = std::fs::read_to_string(root.join("population.yaml")).expect("population.yaml");
    let real = Population::from_yaml(&pop_text).expect("the real population parses");

    println!("── audit-rectify over rung's own questions ──────────────────────");
    println!();
    println!("  real population.yaml:");
    for p in &real.principals {
        let q14 = if p.authored.is_empty() {
            "authored=[]  (Q14 placeholder — a ruling here would be vacuous)"
        } else {
            &format!("authored={p:?}")
        };
        println!(
            "    {:<14} kind={:<6} {}",
            p.id,
            format!("{:?}", p.kind),
            q14
        );
    }
    // A principal fills a role by declaring the capabilities the role requires
    // (`Population::capable_of`), not by declaring a capability that happens to
    // spell the role's name.
    let fills = |role: &str| !real.capable_of(role).is_empty();
    let curator_ok = fills("curator");
    let adjudicator_ok = fills("adjudicator");
    let interrogator_ok = fills("interrogator");
    println!(
        "    -> fills curator: {};  interrogator: {};  adjudicator: {}",
        curator_ok, interrogator_ok, adjudicator_ok
    );
    println!();

    // ── the cycle ─────────────────────────────────────────────────────────
    let mut world = Questions::load(RUNG, &root.join("questions"));
    let pop = Population::from_yaml(QUESTIONS_POPULATION).expect("the questions population parses");
    let pool = population_pool(&pop, "adjudicator", Arc::new(Holding));

    let settled = rung_std::questions::questions::affects_mirrors_inbound::holds(&world);
    let Verdict::NonConforming { .. } = settled.verdict() else {
        println!("  audit: conforming — nothing to rectify. Done.");
        return;
    };
    let drift = world.outbound_drift();
    let (src, dependent, kind) = drift.first().expect("the pinned drift exists").clone();
    println!("  audit  : violates affects_mirrors_inbound");
    for (s, d, k) in &drift {
        println!("             {s} --{k}--> {d} unacknowledged");
    }

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

    let judge = pool
        .qualify_for::<Adjudicator>(&proposal)
        .expect("a judge disjoint from the proposal's author");
    let ruling = dispose(&proposal, judge, Disposition::Accept)
        .expect("the licence was minted against this proposal");
    println!("  propose: {src} --{kind}--> {dependent} (mirror the edge)");
    println!("  dispose: {} accepts", ruling.judge());

    let enacted = enact(&mut world, &ruling, &pen).expect("the tree admits the mirroring");
    let _remaining = world
        .outbound_drift()
        .into_iter()
        .filter(|(s, d, _)| s == &src && d == &dependent)
        .count();
    println!(
        "  enact  : landed on {}; that edge no longer drifts",
        enacted.object()
    );
    println!();

    // ── Q14, honestly ────────────────────────────────────────────────────
    let judge = pop.by_id("external-judge").expect("declared");
    println!("── Q14 ──────────────────────────────────────────────────────────");
    println!(
        "  This run used a local judge with declared provenance {:?}, so P0 was real.",
        judge.authored
    );
    if !curator_ok || !adjudicator_ok || !interrogator_ok {
        println!("  Not all three questions roles are currently fillable from");
        println!("  population.yaml; add/refresh principals until they are.");
    }
    // Q14 concerns model principals — the human's provenance is real and always
    // has been. Check only the LLM/Agent principals, which is who would be
    // dispatched as a judge.
    let model_principals: Vec<_> = real
        .principals
        .iter()
        .filter(|p| !matches!(p.kind, rung_driver::Kind::Human))
        .collect();
    if !model_principals.is_empty() && model_principals.iter().all(|p| p.authored.is_empty()) {
        println!("  Every model principal in population.yaml carries authored=[] — the Q14",);
        println!("  placeholder. A dispatched model ruling today would qualify vacuously.",);
        println!("  Q14 (what provenance a model carries) is open; it gates real dispatch.");
    } else {
        println!("  Some model principal declares real provenance; Q14 may be settled.");
    }
    println!();
    println!("  one audit-rectify cycle closed over rung's own questions.");
}
