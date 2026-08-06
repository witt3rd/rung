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

use rung_driver::{
    Answer, Backing, CommissionLog, CycleOutcome, Oracle, Population, population_pool, run_cycle,
};

use rung_std::questions::{Questions, Scheme};
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

    let log_text =
        std::fs::read_to_string(root.join("commissions.yaml")).expect("commissions.yaml");
    let real_log = CommissionLog::from_yaml(&log_text).expect("the commission record parses");

    println!("── audit-rectify over rung's own questions ──────────────────────");
    println!();
    println!("  real population.yaml (authored derived from commissions.yaml):");
    for p in &real.principals {
        // A principal with a family derives its stake from the record; a
        // principal without one (the human) carries its own genuine record.
        let prov = match &p.family {
            Some(fam) => format!(
                "family={fam}  ->  authored={:?}",
                real_log.artifacts_for(fam)
            ),
            None => format!("authored={:?}", p.authored),
        };
        println!(
            "    {:<14} kind={:<6} {}",
            p.id,
            format!("{:?}", p.kind),
            prov
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

    // ── the cycle — the composed loop, run by the driver (not hand-rolled) ──
    let mut world = Questions::load(RUNG, &root.join("questions"));
    let pop = Population::from_yaml(QUESTIONS_POPULATION).expect("the questions population parses");
    let pool = population_pool(&pop, "adjudicator", Arc::new(Holding));

    match run_cycle(&mut world, &pop, &pool, Arc::new(Holding)) {
        CycleOutcome::Clean => {
            println!("  audit: conforming — nothing to rectify. Done.");
            return;
        }
        CycleOutcome::Rectified { verified, record } => {
            println!("  audit  : violates affects_mirrors_inbound");
            for (s, d, k) in &world.outbound_drift() {
                println!("             {s} --{k}--> {d} unacknowledged");
            }
            println!("  propose/dispose/enact closed the cycle");
            println!(
                "  verify : {} (the observer read the post-state back, not the author's word)",
                verified
            );
            println!(
                "  record : {} — {} by {}",
                record.proposition, record.tier, record.judges[0].id
            );
            println!(
                "  provenance: {:?} (out of the sealed judgment)",
                record.judges[0].provenance
            );
        }
    }

    // ── provenance, honestly ─────────────────────────────────────────────
    let judge = pop.by_id("external-judge").expect("declared");
    println!("── provenance ───────────────────────────────────────────────────");
    println!(
        "  This run used a local judge with declared provenance {:?}, so P0 was real.",
        judge.authored
    );
    if !curator_ok || !adjudicator_ok || !interrogator_ok {
        println!("  Not all three questions roles are currently fillable from");
        println!("  population.yaml; add/refresh principals until they are.");
    }
    // Q14/Q16/Q17: model provenance is now DERIVED by family from the
    // commission record (Q16's carrier, Q17 built). It is real the moment the
    // record contains a contribution; today it is empty, so every model set is
    // open — not the refused per-invocation vacuity, but the honest "no
    // commission recorded yet" state.
    let family_principals: Vec<&rung_driver::PrincipalSpec> = real
        .principals
        .iter()
        .filter(|p| p.family.is_some())
        .collect();
    if family_principals.is_empty() {
        println!("  No model principal declares a `family`; nothing derives provenance.");
    } else if family_principals
        .iter()
        .all(|p| !real_log.has_authored(p.family.as_deref().unwrap()))
    {
        println!("  Every model principal derives authored from commissions.yaml by family,",);
        println!("  and the record currently records no contributions, so all derived",);
        println!("  sets are open. Real (non-vacuous) dispatch begins the moment the",);
        println!("  record attributes work to a family — the mechanism is wired.");
    } else {
        println!("  Some model family has recorded contributions; dispatch is non-vacuous.");
    }
    println!();
    println!("  one audit-rectify cycle closed over rung's own questions.");
}
