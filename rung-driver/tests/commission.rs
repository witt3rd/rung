//! The commission contribution record — Q16's carrier, Q17's build.
//!
//! `authored(p)` is **derived by lookup** from this record, keyed on a
//! principal's family. The family-indexed shape is the one Q16 ruled on and it
//! reads as a lookup rather than a per-principal growing array: a population
//! declares a stable `family` per model, and the record supplies the artifacts
//! that family produced.
//!
//! What is proven here is the mechanism — the three conditions, the refusal to
//! be a guessed array, the end-to-end disjointness. Whether rung's own record
//! is *populated* is harness state, not this file's business; this file shows
//! what the record *does* the moment it holds a genuine contribution.

use rung::{Prov, Provenanced, QualifyError, Role, Situated};
use rung_driver::{
    Answer, Backing, CommissionLog, ConfigError, Oracle, Roster, population_pool_with_log,
};
use std::sync::Arc;

// ── a domain, as the driver sees it ────────────────────────────────────────

#[derive(Clone, Copy)]
struct Judge;
impl Role for Judge {
    const NAME: &'static str = "judge";
}

#[derive(Clone)]
struct Subject {
    id: &'static str,
}
impl Provenanced for Subject {
    fn provenance(&self) -> Prov {
        Prov::of([self.id])
    }
}
impl Situated for Subject {
    fn container(&self) -> &str {
        "docs"
    }
}

struct Answering;
impl Oracle for Answering {
    fn ask(&self, _id: &str, _backing: &Backing, _matter: &str) -> Answer {
        Answer::holds()
    }
}

/// Two families, one declared judge each. `family-a` produced `artifact-of-a`
/// and `family-b` produced `artifact-of-b`, under an active commission.
const POP: &str = r#"
roles:
  - name: judge
    requires: [reasoning, structured-outputs]
principals:
  - id: fam-a-judge
    kind: llm
    capabilities: [reasoning, structured-outputs]
    family: family-a
    backing: {via: outside}
  - id: fam-b-judge
    kind: llm
    capabilities: [reasoning, structured-outputs]
    family: family-b
    backing: {via: outside}
"#;

fn symmetric_log() -> CommissionLog {
    CommissionLog::from_yaml(
        r#"
active: [comm-1]
contributions:
  family-a:
    comm-1: [artifact-of-a]
  family-b:
    comm-1: [artifact-of-b]
"#,
    )
    .unwrap()
}

// ═══════════════════════════════════════════════════════════════════════════
// 1 · The derivation is a lookup across active commissions
// ═══════════════════════════════════════════════════════════════════════════

/// `authored(f) = ⋃_{c∈active} C(f,c)`, deduplicated and order-stable.
#[test]
fn authored_is_the_union_of_a_familys_active_commissions() {
    let log = CommissionLog::from_yaml(
        r#"
active: [comm-2, comm-1]
contributions:
  family-a:
    comm-1: [x, y]
    comm-2: [y, z]
"#,
    )
    .unwrap();

    // `y` in both commissions is the same artifact, deduplicated once.
    assert_eq!(log.artifacts_for("family-a"), vec!["x", "y", "z"]);
    assert!(log.has_authored("family-a"));
}

/// **Not total.** An artifact in a closed, non-carried-forward commission is
/// NOT in `authored` — it falls out of the active set and re-opens to later
/// instances of the same family. Only an explicit carry-forward brings a prior
/// commission's artifacts back.
#[test]
fn closed_non_carried_commissions_stay_open() {
    let log = CommissionLog::from_yaml(
        r#"
active: [comm-2]
contributions:
  family-a:
    comm-1: [old-work]
    comm-2: [current-work]
"#,
    )
    .unwrap();

    // comm-1 is closed and not carried forward: its artifact is open again.
    assert_eq!(log.artifacts_for("family-a"), vec!["current-work"]);

    // Supply carries comm-1 forward -> its artifacts return to `authored`.
    let mut carried = log.clone();
    carried.active.push("comm-1".into());
    assert_eq!(
        carried.artifacts_for("family-a"),
        vec!["current-work", "old-work"]
    );
}

/// **A new commission is empty** for every family — no retroactive claim.
#[test]
fn a_new_commission_starts_empty() {
    let log = CommissionLog::from_yaml("active: [comm-new]\ncontributions: {}\n").unwrap();
    assert!(log.artifacts_for("family-a").is_empty());
    assert!(!log.has_authored("family-a"));
}

/// The record is data in a file, and round-trips — a record a driver reads and
/// the one it re-serializes cannot drift.
#[test]
fn the_record_round_trips_through_yaml() {
    let log = symmetric_log();
    let text = serde_yaml::to_string(&log).unwrap();
    let back = CommissionLog::from_yaml(&text).unwrap();
    assert_eq!(log, back);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2 · Non-vacuity lands, end to end, through the pool
// ═══════════════════════════════════════════════════════════════════════════

/// The point of the whole carrier: a family that recorded a contribution can
/// no longer judge it (P0 is non-vacuous), while judging something else is
/// untouched. Each family is asked alone, so the only way to get a licence for
/// its own artifact would be a fabricated set — there is no other judge to
/// fall back on.
fn single_family_pool(family: &str) -> rung::Pool<rung_driver::Configured<Answering>> {
    let pop = Roster::from_yaml(&format!(
        r#"
roles:
  - name: judge
    requires: [reasoning]
principals:
  - id: {family}-judge
    kind: llm
    capabilities: [reasoning]
    family: {family}
    backing: {{via: outside}}
"#
    ))
    .unwrap();
    population_pool_with_log(
        &pop,
        "judge",
        Arc::new(Answering),
        Arc::new(symmetric_log()),
    )
}

#[test]
fn a_family_cannot_judge_what_it_produced_but_can_judge_elsewhere() {
    // family-a produced artifact-of-a: refused for it, qualifying for b's.
    let a = single_family_pool("family-a");
    match a
        .qualify_for::<Judge>(&Subject {
            id: "artifact-of-a",
        })
        .unwrap_err()
    {
        QualifyError::NonIdentityViolated { principal, .. } => {
            assert_eq!(principal, "family-a-judge");
        }
        other => panic!("expected the non-identity refusal, got {other}"),
    }
    let q = a
        .qualify_for::<Judge>(&Subject {
            id: "artifact-of-b",
        })
        .expect("family-a is disjoint from family-b's work");
    assert_eq!(q.principal_id(), "family-a-judge");

    // Symmetric for family-b.
    let b = single_family_pool("family-b");
    match b
        .qualify_for::<Judge>(&Subject {
            id: "artifact-of-b",
        })
        .unwrap_err()
    {
        QualifyError::NonIdentityViolated { principal, .. } => {
            assert_eq!(principal, "family-b-judge");
        }
        other => panic!("expected the non-identity refusal, got {other}"),
    }
    let q = b
        .qualify_for::<Judge>(&Subject {
            id: "artifact-of-a",
        })
        .expect("family-b did not produce artifact-of-a");
    assert_eq!(q.principal_id(), "family-b-judge");
}

/// The pool derives `authored` from the record, not from any `authored` field
/// in the population: the declared principals carry `family` and nothing else,
/// and the disjointness above could only come from the log being read.
#[test]
fn family_principals_declare_no_standing_authored() {
    let pop = Roster::from_yaml(POP).unwrap();
    for p in &pop.principals {
        assert!(p.family.is_some());
        assert!(
            p.provenance.is_empty(),
            "a family principal must derive its stake, not declare it"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3 · Refusals — the record and the declaration stay honest
// ═══════════════════════════════════════════════════════════════════════════

/// A principal that declares both a `family` and a static `authored` is
/// refused: two sources of truth for the same fact is exactly what the carrier
/// exists to remove, and the configuration names it as a fault.
#[test]
fn family_plus_authored_is_a_fault() {
    let pop = Roster::from_yaml(
        r#"
roles:
  - name: judge
    requires: [reasoning]
principals:
  - id: both
    kind: llm
    capabilities: [reasoning]
    family: family-a
    authored: [hand-maintained]
    backing: {via: outside}
"#,
    )
    .unwrap();
    assert!(
        pop.check()
            .iter()
            .any(|e| matches!(e, ConfigError::FamilyWithAuthored { id } if id == "both"))
    );
}
