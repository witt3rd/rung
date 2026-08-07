//! The principals convergence — ONE theory, many carriers.
//!
//! There are no longer two notions of "principal". `rung_std::principals` is the
//! single `Roster`/`PrincipalDecl`/`RoleSpec` model, and `population.yaml` — the
//! deployment carrier rung's own dispatch reads — loads into that *same* model
//! via [`Roster::from_yaml`]. The audit half (the principals theory's sentences)
//! and the dispatch half (`population_pool_with_log`) therefore read one source
//! of truth, and `provenance` on the dispatch side is the same `π` the theory
//! audits.
//!
//! This file proves the join: load the real carrier, then show both halves
//! operating over the very same value.

use rung_driver::{Answer, Backing, CommissionLog, Oracle, population_pool_with_log};
use rung_std::principals::{Roster, principal, roster};
use std::sync::Arc;

fn ws_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-driver sits in the workspace")
        .to_path_buf()
}

fn real_roster() -> Roster {
    let root = ws_root();
    let text = std::fs::read_to_string(root.join(".het/rung-questions/population.yaml"))
        .expect("population.yaml");
    Roster::from_yaml(&text).expect("the real population loads into the unified model")
}

fn real_log() -> CommissionLog {
    let root = ws_root();
    let text = std::fs::read_to_string(root.join(".het/rung-questions/commissions.yaml"))
        .expect("commissions.yaml");
    CommissionLog::from_yaml(&text).expect("the commission record parses")
}

struct Answering;
impl Oracle for Answering {
    fn ask(&self, _id: &str, _backing: &Backing, _matter: &str) -> Answer {
        Answer::holds()
    }
}

/// The deployment carrier loads into the theory's own model, and the theory's
/// *structural* sentences hold over it (ids unique, every derived play both
/// declared and earned) — the audit half reads the real population.
#[test]
fn the_theory_audits_the_real_population_through_one_model() {
    let roster = real_roster();

    // It is the theory's model, not a driver copy.
    let _: &Roster = &roster;

    // The structural roster sentences hold over the real carrier: plays are
    // derived from the roster's own role vocabulary at load, so every played
    // role is declared, and ids are unique.
    assert!(
        roster::ids_are_unique::holds(&roster)
            .verdict()
            .is_conforming()
    );
    assert!(
        roster::every_played_role_is_declared::holds(&roster)
            .verdict()
            .is_conforming()
    );
    for p in &roster.principals {
        assert!(
            principal::roles_are_earned::holds(p)
                .verdict()
                .is_conforming(),
            "{} plays a role it has not earned",
            p.id
        );
    }

    // The roster discovered real capability from the carrier: the questions
    // theory's roles are fillable.
    assert!(!roster.capable_of("curator").is_empty());
    assert!(!roster.capable_of("adjudicator").is_empty());
}

/// Dispatch derives `provenance` from the commission record over the **same**
/// roster the theory audits — one source of truth, both halves. A model with a
/// family carries no static `authored`; the pool reads `π` from the log.
#[test]
fn dispatch_and_audit_read_one_roster() {
    let roster = real_roster();
    let log = real_log();

    let pool = population_pool_with_log(&roster, "adjudicator", Arc::new(Answering), Arc::new(log));
    // The adjudicator role is fillable from the real population (non-empty pool), and
    // the models in it derive their stake from the commission record rather
    // than a declaration.
    assert!(!pool.is_empty());

    // Every principal that names a family derives `authored` from the record's
    // active commissions; none carries a hand-maintained list (the refusal of a
    // second source of truth).
    for p in &roster.principals {
        if p.family.is_some() {
            assert!(
                p.provenance.is_empty(),
                "{} must not declare static authored",
                p.id
            );
        }
    }
}
