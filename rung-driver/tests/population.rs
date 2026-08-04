//! Admission is by capability, and by nothing else.
//!
//! Most of what follows checks things the driver **refuses to do**: read kind,
//! read backing, prefer one qualifying principal over another. Each of those
//! would pass a happy-path test while making a filter mean something other than
//! it says.

use rung::{Prov, Provenanced, Role, Situated, Steward};
use rung_driver::{Answer, Backing, Oracle, Population, Unwired, population_pool};
use std::sync::Arc;

// ── the domain's roles ──────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Judge;
impl Role for Judge {
    const NAME: &'static str = "judge";
}

#[derive(Clone, Copy)]
struct Author;
impl Role for Author {
    const NAME: &'static str = "author";
}

// ── a population, declared as a driver would read it ────────────────────────

const POPULATION: &str = r#"
providers:
  - name: somewhere
    base_url: https://example.invalid/v1
    api_key_env: EXAMPLE_KEY

roles:
  - name: judge
    requires: [reasoning, structured-outputs]
  - name: author
    requires: [code-generation, file-editing]

principals:
  # A bare model that declares it can edit files. Capability-first means this
  # is an author; kind-first would have refused it for not being an agent.
  - id: careful-model
    kind: llm
    capabilities: [reasoning, structured-outputs, code-generation, file-editing]
    standing: [docs]
    authored: []
    backing: {via: model, provider: somewhere, model: some-model}

  # An agent wielding tools that does NOT declare structured outputs, so it is
  # not a judge — being an agent buys it nothing.
  - id: tool-agent
    kind: agent
    capabilities: [reasoning, code-generation, file-editing, web-research]
    standing: [docs]
    authored: [a-thing-it-wrote]
    backing: {via: agent, provider: somewhere, model: some-model, tools: [edit, search]}

  # Reachable, but declares only reasoning.
  - id: a-person
    kind: human
    capabilities: [reasoning]
    standing: [docs]
    authored: []
"#;

fn population() -> Population {
    Population::from_yaml(POPULATION).expect("the population parses")
}

// ── the subject ─────────────────────────────────────────────────────────────

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

// ════════════════════════════════════════════════════════════════════════════
// 1 · Capability admits
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn a_role_is_filled_by_whoever_declares_what_it_requires() {
    let p = population();
    // `web-research` is declared by `tool-agent` and required by no role, and
    // `check` says so. That is a report, not a fault — the population is well
    // formed and one declaration is inert, which is worth seeing.
    assert!(
        p.check()
            .iter()
            .all(|e| matches!(e, rung_driver::ConfigError::Unused { .. })),
        "{:?}",
        p.check()
    );

    let judges: Vec<&str> = p
        .capable_of("judge")
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    let authors: Vec<&str> = p
        .capable_of("author")
        .iter()
        .map(|s| s.id.as_str())
        .collect();

    assert_eq!(judges, ["careful-model"]);
    assert_eq!(authors, ["careful-model", "tool-agent"]);
}

/// **Kind is not read.** `careful-model` is an `llm` and fills `author`, whose
/// requirements include `file-editing`; `tool-agent` is an `agent` and does not
/// fill `judge`, because it never declared `structured-outputs`.
///
/// Mutation: filter `capable_of` by kind — on any reading of which kind "should"
/// do what — and one of these two assertions breaks.
#[test]
fn kind_decides_nothing() {
    let p = population();

    let model = p.by_id("careful-model").unwrap();
    let agent = p.by_id("tool-agent").unwrap();

    assert_eq!(model.kind, rung_driver::Kind::Llm);
    assert!(
        p.capable(model, "author"),
        "a model that declares file-editing fills a role needing it"
    );

    assert_eq!(agent.kind, rung_driver::Kind::Agent);
    assert!(
        !p.capable(agent, "judge"),
        "an agent that never declared structured-outputs does not fill a role needing it"
    );
}

/// Backing is not read either. The same capabilities admit whether the
/// principal is reached by a model call, an agentic turn, or out of band.
#[test]
fn backing_decides_nothing() {
    let mut p = population();
    let before = p.capable_of("author").len();

    for spec in &mut p.principals {
        spec.backing = Backing::Outside;
    }
    assert_eq!(p.capable_of("author").len(), before);
}

/// A role nobody declared admits **nobody**, not everybody.
///
/// The failure worth guarding: "requirements not found" reading as "no
/// requirements", so an unknown role is filled by the first principal in the
/// file. That is a filter that cannot fail.
#[test]
fn an_undeclared_role_admits_nobody() {
    let p = population();
    assert!(p.capable_of("archivist").is_empty());
    assert!(!p.capable(p.by_id("careful-model").unwrap(), "archivist"));
}

/// A role requiring nothing is filled by everyone — which is what it asked for.
/// Stated so that the previous test is understood as being about *absence*
/// rather than about emptiness.
#[test]
fn a_role_requiring_nothing_admits_everyone() {
    let mut p = population();
    p.roles.push(rung_driver::RoleSpec {
        name: "observer".into(),
        requires: vec![],
    });
    assert_eq!(p.capable_of("observer").len(), p.principals.len());
}

// ════════════════════════════════════════════════════════════════════════════
// 2 · The pool applies what capability cannot
// ════════════════════════════════════════════════════════════════════════════

/// Non-identity is applied per **argument**, not per principal — so a principal
/// is capable in general and refused for the one thing it wrote.
#[test]
fn a_capable_principal_is_still_refused_for_what_it_authored() {
    let p = population();
    let pool = population_pool(&p, "author", Arc::new(Answering));

    // `tool-agent` authored `a-thing-it-wrote` and is capable of `author`.
    assert!(p.capable(p.by_id("tool-agent").unwrap(), "author"));

    // Judged against something else, a qualifying principal is found.
    assert!(
        pool.qualify_for::<Author>(&Subject {
            id: "something-else"
        })
        .is_ok()
    );

    // The pool holds two authors and only one is disjoint from this subject,
    // so a licence is still minted — by the other one.
    let q = pool
        .qualify_for::<Author>(&Subject {
            id: "a-thing-it-wrote",
        })
        .expect("careful-model authored nothing");
    assert_eq!(q.principal_id(), "careful-model");
}

/// Standing is separate from capability. `capable` says what a principal can
/// do; `has_standing` says where it may write, and the authorial filter needs
/// both.
#[test]
fn capability_alone_does_not_authorize_a_write() {
    let mut p = population();
    for spec in &mut p.principals {
        spec.standing.clear();
    }
    let pool = population_pool(&p, "author", Arc::new(Answering));
    let subject = Subject { id: "anything" };

    // Still capable...
    assert_eq!(p.capable_of("author").len(), 2);
    // ...and still unable to write anywhere.
    let principal = rung_driver::Configured::new(
        p.by_id("careful-model").unwrap().clone(),
        Arc::new(Answering),
    );
    assert!(!principal.has_standing("docs"));
    assert!(pool.authorize::<Author, _>(&principal, "docs").is_err());
    let _ = subject;
}

// ════════════════════════════════════════════════════════════════════════════
// 3 · What the driver refuses to decide
// ════════════════════════════════════════════════════════════════════════════

/// **Any qualifying principal, not the best one.** Two authors qualify for a
/// subject neither wrote, and the pool returns one without ranking them.
///
/// There is nothing here to assert about *which*, and that is the assertion:
/// the driver exposes no way to express a preference, so a caller wanting the
/// cheapest cannot get it from this layer — which is where the worth law would
/// otherwise arrive.
#[test]
fn the_driver_offers_no_way_to_prefer_one_qualifying_principal() {
    let p = population();
    let pool = population_pool(&p, "author", Arc::new(Answering));
    assert_eq!(pool.len(), 2, "both authors are in the pool");

    let q = pool
        .qualify_for::<Author>(&Subject { id: "untouched" })
        .expect("both qualify");
    assert!(["careful-model", "tool-agent"].contains(&q.principal_id()));
}

/// An unwired driver **raises a matter** rather than affirming.
///
/// The alternative — defaulting to conforming when nothing is connected — is a
/// system that reports success for work nobody did.
#[test]
fn an_unwired_oracle_defers_rather_than_agreeing() {
    let p = population();
    let pool = population_pool(
        &p,
        "judge",
        Arc::new(Unwired {
            reference: "no-oracle-configured".into(),
        }),
    );

    match pool.qualify_for::<Judge>(&Subject { id: "a-subject" }) {
        Err(rung::QualifyError::JudgeDeferred(raised)) => {
            assert_eq!(raised.reference(), "no-oracle-configured");
        }
        Err(other) => panic!("expected a deferral, got {other}"),
        Ok(_) => panic!("an unwired driver produced a verdict"),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 4 · The declaration itself
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn a_duplicate_declaration_is_reported() {
    let mut p = population();
    p.principals.push(p.principals[0].clone());
    assert!(matches!(
        p.check().first(),
        Some(rung_driver::ConfigError::DuplicatePrincipal { .. })
    ));
}

/// A capability no role asks for is reported — not as an error, but so that a
/// declaration nothing reads is visible instead of silently inert.
#[test]
fn a_capability_no_role_requires_is_reported() {
    let mut p = population();
    p.principals[0].capabilities.push("telepathy".into());
    let errs = p.check();
    assert!(errs.iter().any(|e| matches!(
        e,
        rung_driver::ConfigError::Unused { capability, .. } if capability == "telepathy"
    )));
}

/// The population round-trips through YAML, so a driver's config file and its
/// in-memory form cannot drift.
#[test]
fn a_population_round_trips_through_yaml() {
    let p = population();
    let text = serde_yaml::to_string(&p).expect("serializes");
    let back = Population::from_yaml(&text).expect("parses");
    assert_eq!(p, back);
}
