//! The composed audit-rectify cycle, run by the driver (Q19).
//!
//! One cycle over rung's own questions: audit finds the pinned `affects_
//! mirrors_inbound` drift, an author proposes mirroring one edge, a disjoint
//! judge accepts, enact lands it (in-memory), the observer verifies the
//! post-state — and the ruling's sealed `Judgment` is written as a
//! `tier: dispatched` record. This is the loop the hand-rolled
//! `rectify_questions` used to spell out by hand.

use rung_driver::{
    Answer, Backing, Configured, CycleOutcome, Oracle, Roster, population_pool, run_cycle,
};
use rung_std::questions::{Adjudicator, Curator, Questions, Scheme};
use std::sync::Arc;

const RUNG: Scheme = Scheme {
    namespace: "rung-questions",
    root: "questions",
    id_prefix: "q",
};

struct Holding;
impl Oracle for Holding {
    fn ask(&self, _id: &str, _backing: &Backing, _matter: &str) -> Answer {
        Answer::holds()
    }
}

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

fn ws_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn the_composed_loop_closes_with_a_dispatched_record() {
    let mut world = Questions::load(RUNG, &ws_root().join(".het/rung-questions/questions"));
    let pop = Roster::from_yaml(QUESTIONS_POPULATION).unwrap();
    let pool = population_pool(&pop, "adjudicator", Arc::new(Holding));

    let author = pop.by_id("opus-author").expect("declared").clone();
    let author_cfg = Configured::new(author, Arc::new(Holding));
    match run_cycle::<_, _, _, Curator, Adjudicator>(&mut world, author_cfg, "questions", &pool) {
        CycleOutcome::Clean => {
            panic!("the pinned drift is genuinely present; the loop must not report clean")
        }
        CycleOutcome::Rejected { reasons } => {
            panic!("the Holding oracle conforms; judging must affirm, not reject: {reasons:?}")
        }
        CycleOutcome::Deferred => {
            panic!("the Holding oracle never defers; judging must not await anything")
        }
        CycleOutcome::Rectified { verified, record } => {
            // verify: the observer reads the enacted edge back from the state
            assert!(
                verified,
                "after enact the edge must be observably in effect"
            );
            // bookkeeping: a `tier: dispatched` record, provenance from the seal
            assert_eq!(record.tier, "dispatched");
            assert_eq!(record.role, "adjudicator");
            assert_eq!(record.judges.len(), 1);
            assert_eq!(record.judges[0].id, "external-judge");
            // π(p) = authored ∪ {id}, straight out of the sealed judgment
            assert_eq!(
                record.judges[0].provenance,
                vec!["external-attestation", "external-judge"]
            );
            assert_eq!(record.judges[0].verdict, "conforming");
        }
    }
}
