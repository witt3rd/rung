//! Generic Intake / Discharge: admit gated on the destination's audit, and
//! discharge, over a real folder carrier.
//!
//! The destination is the Issues theory (`rung-std::issues`), demonstrating
//! that admission is gated on *the destination's* membership — the source may
//! say "a work item, not a question," but only the Issues theory decides
//! whether the candidate is a well-formed *issue*.

use rung_driver::{Carrier, FolderCarrier, ObjectId, admit, discharge};
use rung_std::issues::{Issue, Issues, Scheme};

const SCHEME: Scheme = Scheme {
    namespace: "rung-issues",
    root: "issues",
    id_prefix: "issue",
};

fn issue_content(id: &str, status: &str, body: &str) -> String {
    format!(
        "---
id: {id}
status: {status}
---

{body}
"
    )
}

#[test]
fn admit_is_gated_on_the_destinations_membership() {
    let dir = tempdir("admit-gated");
    let carrier = FolderCarrier::new(&dir);

    // An empty Issues world — the host whose audit is the intake gate.
    let host = Issues::new(SCHEME, vec![]);
    let id = ObjectId::new("rung-1");

    // A candidate that IS a well-formed issue: admitted, persisted to the carrier.
    let candidate = issue_content("rung-1", "open", "The work item for q-7.");
    let assigned =
        admit(&host, &carrier, &id, &candidate).expect("a well-formed issue is admitted");
    assert!(
        assigned.as_str().ends_with("/rung-1.md"),
        "carrier writes rung-1.md"
    );

    // The carrier yields the admitted subject, and it re-parses as an open issue.
    let walked: Vec<ObjectId> = carrier.iter().map(|r| r.unwrap()).collect();
    assert!(walked.iter().any(|o| o.as_str().ends_with("/rung-1.md")));
    let text = carrier.read(&assigned).unwrap();
    let it = Issue::parse(SCHEME, &text, "", "rung-1").unwrap();
    assert_eq!(it.status, "open");
    assert_eq!(it.id, "rung-1");

    // Admission verifies through the model, too.
    let reloaded = Issues::load(SCHEME, &dir);
    assert!(reloaded.by_id("rung-1").is_some());
    assert_eq!(reloaded.by_id("rung-1").unwrap().status, "open");
}

#[test]
fn admit_refuses_a_candidate_that_fails_the_gate() {
    let dir = tempdir("admit-refused");
    let carrier = FolderCarrier::new(&dir);
    let host = Issues::new(SCHEME, vec![]);

    // No `status:` frontmatter -> not a well-formed issue -> refused, carrier untouched.
    let bad = "---
id: rung-2
---

Not an issue: no status declared.
";
    let err = admit(&host, &carrier, &ObjectId::new("rung-2"), bad).unwrap_err();
    match err {
        rung_driver::IntakeError::Refused { .. } => {}
        other => panic!("expected Refused, got {other}"),
    }
    let walked: Vec<ObjectId> = carrier.iter().map(|r| r.unwrap()).collect();
    assert!(
        walked.is_empty(),
        "a refused candidate never touches the carrier"
    );
}

#[test]
fn discharge_removes_a_subject_from_the_carrier() {
    let dir = tempdir("discharge");
    let carrier = FolderCarrier::new(&dir);
    let host = Issues::new(SCHEME, vec![]);
    let id = ObjectId::new("rung-3");
    admit(
        &host,
        &carrier,
        &id,
        &issue_content("rung-3", "open", "Item to remove."),
    )
    .unwrap();

    // Walk to the carrier-assigned id, then discharge it.
    let assigned: ObjectId = carrier
        .iter()
        .map(|r| r.unwrap())
        .find(|o| o.as_str().ends_with("rung-3.md"))
        .unwrap();
    discharge(&carrier, &assigned).expect("discharge removes the subject");
    let walked: Vec<ObjectId> = carrier.iter().map(|r| r.unwrap()).collect();
    assert!(
        !walked.iter().any(|o| o.as_str().ends_with("rung-3.md")),
        "subject is gone from the carrier"
    );
    let reloaded = Issues::load(SCHEME, &dir);
    assert!(reloaded.by_id("rung-3").is_none());
}

fn tempdir(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("rung-intake-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}
