//! The cycle treats judging abstractly — one judge or a panel acting as one.
//!
//! `[judging]` is one step of the audit-rectify cycle, and rung doesn't care
//! how many principals perform it: whether the theory's judgmental seat is a
//! sole judge or a panel is conceptually irrelevant to the loop. This file
//! shows both over the same `run_cycle`, on rung's own questions:
//!
//! - a **divergent panel** — two experts agree, a third dissents — does not
//!   affirm; the dissent's reason surfaces so the author re-proposes with it
//!   (`reject-remedy` carries a reason, 7.43; `panels-cannot-weaken-the-opponent`);
//! - a **consensus panel** — every expert agrees — affirms and enacts, and the
//!   `dispatched` record lists every judge with each one's sealed provenance;
//! - a **sole judge** is just a panel of one — the exact same cycle, no
//!   separate mode (that is the point of the abstraction).

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

fn ws_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Three experts, one author. Each expert is a different family, so each is
/// provenance-disjoint from the author's proposal (and from the others).
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
  - id: opus-author
    kind: agent
    capabilities: [reasoning, file-editing]
    standing: [questions]
    authored: [rung-questions]
    backing: {via: outside}
  - id: expert-a
    kind: llm
    capabilities: [reasoning, structured-outputs]
    family: family-a
    backing: {via: outside}
  - id: expert-b
    kind: llm
    capabilities: [reasoning, structured-outputs]
    family: family-b
    backing: {via: outside}
  - id: expert-c
    kind: llm
    capabilities: [reasoning, structured-outputs]
    family: family-c
    backing: {via: outside}
"#;

/// An oracle that can be scripted per expert, so a test can arrange agreement
/// and dissent — the outside speaking, which is the one thing a test is
/// allowed to arrange by choosing whom to ask.
struct Scripted;
impl Oracle for Scripted {
    fn ask(&self, id: &str, _backing: &Backing, _matter: &str) -> Answer {
        match id {
            "expert-c" => Answer::fails(
                "the mirror direction depends on the target's own evidence, which is absent",
            ),
            _ => Answer::holds(),
        }
    }
}

fn world() -> Questions {
    Questions::load(RUNG, &ws_root().join("questions"))
}

fn author_cfg(pop: &Roster) -> Configured<Scripted> {
    let author = pop.by_id("opus-author").expect("declared").clone();
    Configured::new(author, Arc::new(Scripted))
}

/// **Divergence reshapes the resolution.** Two experts agree; a third dissents.
/// The panel does not affirm, and the dissenting reason reaches the author —
/// the exact shape in which "strong agreement on some aspects, divergence on
/// others" redirects what the author next authors.
#[test]
fn a_divergent_panel_does_not_affirm_and_surfaces_the_dissent() {
    let mut w = world();
    let pop = Roster::from_yaml(POPULATION).unwrap();
    let pool = population_pool(&pop, "adjudicator", Arc::new(Scripted));

    // every expert is on the panel — the cycle consults all of them
    assert_eq!(pool.len(), 3);

    match run_cycle::<_, _, _, Curator, Adjudicator>(&mut w, author_cfg(&pop), "questions", &pool) {
        CycleOutcome::Rejected { reasons } => {
            assert_eq!(
                reasons,
                vec!["the mirror direction depends on the target's own evidence, which is absent"]
            );
            // nothing was enacted: the world still carries the pinned drift
            assert!(
                !w.outbound_drift().is_empty(),
                "a rejected cycle must not enact"
            );
        }
        other => panic!("expert-c dissents, so judging must not affirm; got {other:?}"),
    }
}

/// **Consensus closes — and the record carries every judge's sealed
/// provenance.** With a silent agreeing panel the loop enacts and verifies,
/// and the `tier: dispatched` record lists all three experts.
#[test]
fn a_consensus_panel_closes_and_records_every_judge() {
    // a silent oracle: everyone holds
    struct AllHold;
    impl Oracle for AllHold {
        fn ask(&self, _id: &str, _backing: &Backing, _matter: &str) -> Answer {
            Answer::holds()
        }
    }

    let mut w = world();
    let pop = Roster::from_yaml(POPULATION).unwrap();
    let pool = population_pool(&pop, "adjudicator", Arc::new(AllHold));

    let author = pop.by_id("opus-author").unwrap().clone();
    let author_cfg = Configured::new(author, Arc::new(AllHold));

    match run_cycle::<_, _, _, Curator, Adjudicator>(&mut w, author_cfg, "questions", &pool) {
        CycleOutcome::Rectified { verified, record } => {
            assert!(verified, "the observer reads the enacted edge back");
            // the record lists the whole panel, each with its own provenance
            let ids: Vec<&str> = record.judges.iter().map(|j| j.id.as_str()).collect();
            assert_eq!(ids, ["expert-a", "expert-b", "expert-c"]);
            for j in &record.judges {
                assert!(
                    j.provenance.iter().any(|p| p == &j.id),
                    "{}: the sealed provenance carries its floor id",
                    j.id
                );
            }
        }
        other => panic!("unanimous agreement must affirm and enact; got {other:?}"),
    }
}

/// **A sole judge is a panel of one.** Same amount of "judging" to the cycle —
/// this is precisely the abstraction: no separate mode exists for a single
/// judge.
#[test]
fn a_sole_judge_is_a_panel_of_one() {
    struct AllHold;
    impl Oracle for AllHold {
        fn ask(&self, _id: &str, _backing: &Backing, _matter: &str) -> Answer {
            Answer::holds()
        }
    }

    // a population with a single adjudicator-capable expert
    const SOLO: &str = r#"
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
  - {id: opus-author, kind: agent, capabilities: [reasoning, file-editing], standing: [questions], authored: [rung-questions], backing: {via: outside}}
  - {id: solo-judge, kind: llm, capabilities: [reasoning, structured-outputs], family: family-solo, backing: {via: outside}}
"#;
    let mut w = world();
    let pop = Roster::from_yaml(SOLO).unwrap();
    let pool = population_pool(&pop, "adjudicator", Arc::new(AllHold));
    assert_eq!(pool.len(), 1);

    let author = pop.by_id("opus-author").unwrap().clone();
    let author_cfg = Configured::new(author, Arc::new(AllHold));

    match run_cycle::<_, _, _, Curator, Adjudicator>(&mut w, author_cfg, "questions", &pool) {
        CycleOutcome::Rectified { record, .. } => {
            assert_eq!(record.judges.len(), 1);
            assert_eq!(record.judges[0].id, "solo-judge");
        }
        other => panic!("a sole affirming judge must close the cycle; got {other:?}"),
    }
}
