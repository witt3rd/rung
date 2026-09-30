//! The `dispatched` judgment record — the driver's bookkeeping (handoff §2.2).
//!
//! A `dispatched` record is the honest form an `attested` transcription cannot
//! reach: the judge's provenance comes **out of the sealed `Judgment`** (via
//! `Provenanced`), never a field someone typed. This is produced from a real
//! `Pool::consult` of a judgmental sentence — the difference between a receipt
//! and a judgment.

use rung::{Principal, Prov, Provenanced, Rendering, Response, Verdict, VerdictPoint};
use rung_driver::{Answer, Backing, DispatchedRecord, Oracle, Roster, population_pool};
use rung_std::questions::{Interrogator, Question, Scheme};
use std::sync::Arc;

struct Person {
    id: &'static str,
    prov: &'static [&'static str],
    roles: &'static [&'static str],
}
impl Principal for Person {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }
    fn authored(&self) -> Prov {
        Prov::of(self.prov.iter().copied())
    }
    fn rule(&self, _matter: &str) -> Response {
        Response::Rendered(Verdict::Conforming.into())
    }
}

/// A real question (q7 is on disk, flat) — the subject both tests consult.
fn q7() -> Question {
    let scheme = Scheme {
        namespace: "rung-questions",
        root: "questions",
        id_prefix: "q",
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let text = std::fs::read_to_string(
        root.join(".het/rung-questions/questions/q7-effectful-bodies-which-monad.md"),
    )
    .unwrap();
    Question::parse(scheme, &text, "resolved", "q7-effectful-bodies-which-monad")
        .map(|mut q| {
            q.dir = q.status.clone();
            q
        })
        .expect("q7 parses")
}

#[test]
fn a_dispatched_record_carries_the_sealed_provenance() {
    let q7 = q7();

    let pool = rung::Pool::new(vec![
        // an outside reviewer: interrogator-capable, sourced from outside,
        // stewards nothing — disjoint from the questions namespace.
        Person {
            id: "external-reviewer",
            prov: &["external-review"],
            roles: &["interrogator"],
        },
    ]);

    // dispatch the judgmental sentence; the sealed Judgment rides back with us
    let (qualified, judgment) = pool
        .consult::<Interrogator>(&q7, "is_well_posed")
        .expect("the outside reviewer is disjoint from the question");

    // the driver writes the bookkeeping record from the sealed judgment
    let rec =
        DispatchedRecord::from_judgment("is_well_posed", "interrogator", &judgment, "2026-08-06");
    assert_eq!(rec.tier, "dispatched");
    assert_eq!(rec.role, "interrogator");
    assert_eq!(rec.judges.len(), 1);
    assert_eq!(rec.judges[0].id, "external-reviewer");
    // provenance OUT OF THE SEALED JUDGMENT — this is the whole point. It is
    // not a field this writer set; it is what the sealed Judgment carried.
    // π(p) = authored ∪ {id} — the floor adds the id, so the sealed judgment
    // carries both. This is the provenance the record reports, and it is the
    // sealed `Judgment`'s, not a field the writer typed.
    assert_eq!(
        rec.judges[0].provenance,
        vec!["external-review", "external-reviewer"]
    );
    assert_eq!(rec.judges[0].verdict, "conforming");
    // an unweighed judge (every prose judge today) is uncalibrated, and the
    // record says so by carrying no ε — not a made-up one.
    assert_eq!(rec.judges[0].epsilon, None);

    // and it matches the token that was actually licensed for the consult
    assert_eq!(qualified.principal_id(), "external-reviewer");
    assert!(judgment.provenance().contains("external-review"));

    // round-trips to the judgments/ YAML schema shape
    let yaml = serde_yaml::to_string(&rec).unwrap();
    assert!(yaml.contains("tier: dispatched"));
    assert!(yaml.contains("provenance:"));
    assert!(yaml.contains("- external-review"));
    assert!(!yaml.contains("epsilon"));
}

// ── ε, out of the seal ──────────────────────────────────────────────────────
//
// `epsilon-reported-with-verdict`, at the driver's end: a judge whose oracle
// reports a confidence gets a record whose `epsilon` is that confidence's
// error bar, read out of the sealed `Judgment` — the writer never sets it.

const FIXTURE: &str = include_str!("fixtures/weighed/is_well_posed.json");

/// An oracle that answers from a response fixture in Jev's shape (a Choice:
/// `choice`, `probabilities`, `confidence`), never the network. The verdict,
/// the point and the confidence are read out of the fixture; the reason on a
/// refusal is built in code, never parsed from prose.
struct FixtureOracle;

impl Oracle for FixtureOracle {
    fn ask(&self, _id: &str, _backing: &Backing, matter: &str) -> Answer {
        let response: serde_yaml::Value = serde_yaml::from_str(FIXTURE).expect("fixture is JSON");
        let model = response["model"].as_str().expect("model");
        let answer = &response["answers"]["well_posed"];
        let choice = answer["choice"].as_str().expect("choice");
        let confidence = answer["confidence"].as_f64().expect("confidence");
        let masses = answer["probabilities"]
            .as_mapping()
            .expect("probabilities")
            .iter()
            .map(|(k, v)| (k.as_str().unwrap().to_string(), v.as_f64().unwrap()));
        let verdict = match choice {
            "conforming" => Verdict::Conforming,
            other => Verdict::NonConforming {
                reason: format!("`{matter}`: the judge chose `{other}`"),
            },
        };
        Answer::Rendered(
            Rendering::weighed(
                verdict,
                VerdictPoint::simplex(masses).expect("a distribution"),
                confidence,
                model,
            )
            .expect("a well-formed weight"),
        )
    }
}

const WEIGHED_POPULATION: &str = r#"
providers:
  - name: openrouter
    base_url: https://openrouter.invalid/api/v1
    api_key_env: OPENROUTER_API_KEY

roles:
  - name: interrogator
    requires: [reasoning, structured-outputs]

principals:
  - id: weighed-interrogator
    kind: llm
    capabilities: [reasoning, structured-outputs]
    standing: []
    authored: [external-review]
    backing: {via: model, provider: openrouter, model: typesafe/jev-1.13}
"#;

#[test]
fn a_dispatched_record_carries_the_epsilon_the_judge_reported() {
    let q7 = q7();
    let roster = Roster::from_yaml(WEIGHED_POPULATION).expect("the population parses");
    let pool = population_pool(&roster, "interrogator", Arc::new(FixtureOracle));

    let (_qualified, judgment) = pool
        .consult::<Interrogator>(&q7, "is_well_posed")
        .expect("the weighed interrogator is disjoint from the question");

    // The weight rode through `Configured::rule` into the seal unchanged.
    let weight = judgment.weight().expect("the judge reported a weight");
    assert_eq!(weight.model(), "typesafe/jev-1.13-20260917");
    assert_eq!(weight.confidence(), 0.72);
    assert_eq!(
        weight
            .point()
            .as_simplex()
            .expect("a Choice is a point of Δⁿ")["conforming"],
        0.86
    );

    let rec =
        DispatchedRecord::from_judgment("is_well_posed", "interrogator", &judgment, "2026-09-28");
    assert_eq!(rec.judges[0].id, "weighed-interrogator");
    assert_eq!(rec.judges[0].verdict, "conforming");
    let eps = rec.judges[0].epsilon.expect("ε is written from the seal");
    assert_eq!(
        Some(eps),
        judgment.epsilon(),
        "the record's ε is the seal's"
    );
    assert!((eps - 0.28).abs() < 1e-12, "ε = 1 − 0.72, got {eps}");

    // and it survives to the judgments/ YAML schema.
    let yaml = serde_yaml::to_string(&rec).unwrap();
    let back: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
    let written = back["judges"][0]["epsilon"]
        .as_f64()
        .expect("epsilon is written");
    assert!((written - 0.28).abs() < 1e-12, "{yaml}");
}
