//! The `dispatched` judgment record — the driver's bookkeeping (handoff §2.2).
//!
//! A `dispatched` record is the honest form an `attested` transcription cannot
//! reach: the judge's provenance comes **out of the sealed `Judgment`** (via
//! `Provenanced`), never a field someone typed. This is produced from a real
//! `Pool::consult` of a judgmental sentence — the difference between a receipt
//! and a judgment.

use rung::{Principal, Prov, Provenanced, Response, Verdict};
use rung_driver::DispatchedRecord;
use rung_std::questions::{Interrogator, Question, Scheme};

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
        Response::Rendered(Verdict::Conforming)
    }
}

#[test]
fn a_dispatched_record_carries_the_sealed_provenance() {
    let scheme = Scheme {
        namespace: "rung-questions",
        root: "questions",
        id_prefix: "q",
    };
    // a real question (q7 is on disk, flat) — consult its well-posedness
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let text = std::fs::read_to_string(
        root.join(".het/rung-questions/questions/q7-effectful-bodies-which-monad.md"),
    )
    .unwrap();
    let q7 = Question::parse(scheme, &text, "resolved", "q7-effectful-bodies-which-monad")
        .map(|mut q| {
            q.dir = q.status.clone();
            q
        })
        .expect("q7 parses");

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

    // and it matches the token that was actually licensed for the consult
    assert_eq!(qualified.principal_id(), "external-reviewer");
    assert!(judgment.provenance().contains("external-review"));

    // round-trips to the judgments/ YAML schema shape
    let yaml = serde_yaml::to_string(&rec).unwrap();
    assert!(yaml.contains("tier: dispatched"));
    assert!(yaml.contains("provenance:"));
    assert!(yaml.contains("- external-review"));
}
