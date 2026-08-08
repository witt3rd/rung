//! The issues theory over a synthetic work-item docket.

use rung_het::{Applies, Verify};
use rung_std::issues::{Issue, IssueEdit, Issues, STATUSES, Scheme, issue};

const S: Scheme = Scheme {
    namespace: "rung-work",
    root: "issues",
    id_prefix: "w",
};

fn parse(id: &str, status: &str) -> Issue {
    Issue::parse(
        S,
        &format!("---\nid: {id}\nstatus: {status}\n---\nbody of {id}\n"),
        status,
        id,
    )
    .unwrap()
}

#[test]
fn an_issue_parses_and_its_status_turns_under_the_ladder() {
    let mut set = Issues::new(
        S,
        vec![
            parse("w1", "open"),
            parse("w2", "open"),
            parse("w3", "in-progress"),
        ],
    );
    assert_eq!(set.by_id("w1").unwrap().status, "open");
    assert_eq!(set.by_id("w1").unwrap().stem, "w1");

    // status transitions apply and verify
    set.apply("w1", &IssueEdit::Triage { to: "triaged" })
        .unwrap();
    assert!(set.confirms(&IssueEdit::Triage { to: "triaged" }, "w1"));
    set.apply("w1", &IssueEdit::Resolve).unwrap();
    assert!(set.confirms(&IssueEdit::Resolve, "w1"));
    set.apply("w1", &IssueEdit::Close).unwrap();
    assert!(set.confirms(&IssueEdit::Close, "w1"));
    set.apply("w1", &IssueEdit::Reopen).unwrap();
    assert_eq!(set.by_id("w1").unwrap().status, "open");

    // an undeclared status is refused by the target's own law
    let r = set.apply(
        "w2",
        &IssueEdit::Triage {
            to: "does-not-exist",
        },
    );
    assert!(r.is_err(), "an undeclared status must be refused");
}

#[test]
fn the_decidable_issue_sentences_hold_and_well_scoped_is_judgmental() {
    let set = Issues::new(S, vec![parse("w1", "open"), parse("w2", "closed")]);
    for q in &set.issues {
        assert!(
            issue::id_matches_the_filename::holds(q)
                .verdict()
                .is_conforming()
        );
        assert!(
            issue::status_is_declared::holds(q)
                .verdict()
                .is_conforming()
        );
    }
    assert!(set.by_id("w1").unwrap().status != set.by_id("w2").unwrap().status);

    // well_scoped is judgmental (a Reviewer rules it), never decided cold
    assert!(
        issue::SENTENCES
            .iter()
            .any(|(n, g)| *n == "well_scoped" && *g == "judgmental")
    );
    assert!(STATUSES.contains(&"resolved"));
}
