//! Judgment records — what they must carry, and what no record can establish.
//!
//! There are none yet. 47 propositions declare that only a principal can settle
//! them and nobody has ruled, so the interesting tests here are about the
//! **checks**, exercised against records built in memory. A collection whose
//! validation has never been run against a bad record is a validation nobody
//! has tested.

use rung_doctrine::judgment::{Fault, Record, Ruling, Tier, check, read_all};
use rung_doctrine::{Kind, rung, rung_ct, rung_het};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-doctrine sits in the workspace")
        .to_path_buf()
}

fn dir() -> PathBuf {
    root().join("judgments")
}

fn all() -> Vec<rung_doctrine::Doctrine> {
    vec![rung::doctrine(), rung_het::doctrine(), rung_ct::doctrine()]
}

/// A record that should pass every check: a real judgmental proposition, the
/// role it declares, a judge disjoint from it, a verdict, and an argument.
fn sound() -> Record {
    Record {
        proposition: "transition-is-a-prism".into(),
        role: "category-theorist".into(),
        tier: Tier::Attested,
        judges: vec![Ruling {
            id: "a-reader".into(),
            provenance: vec!["something-else".into()],
            verdict: "conforming".into(),
            epsilon: Some("0.1".into()),
            on: "2026-08-05".into(),
        }],
        reasoning: "The forward pass matches, and the backward pass is the declared edge.".into(),
        file: "transition-is-a-prism.md".into(),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 1 · The collection as it stands
// ════════════════════════════════════════════════════════════════════════════

/// **There are no judgments.** Reported rather than asserted at a threshold —
/// the number's job is to be visible and to rise.
///
/// An empty collection with a schema is a better account of the state than a
/// plausible example would be: a fabricated judgment is precisely what this
/// collection exists to make impossible to pass off.
#[test]
fn the_collection_is_empty_and_says_so() {
    let records = read_all(&dir());
    let unsettled = all()
        .iter()
        .flat_map(|d| d.props().cloned().collect::<Vec<_>>())
        .filter(|p| matches!(&p.kind, Kind::Judgmental { ruling: None, .. }))
        .count();
    println!(
        "\n  judgment records: {}\n  unsettled:        {unsettled}\n",
        records.len()
    );
    assert_eq!(
        records.len() + unsettled,
        all()
            .iter()
            .flat_map(|d| d.props())
            .filter(|p| matches!(p.kind, Kind::Judgmental { .. }))
            .count(),
        "every judgmental proposition is either settled by a record or unsettled"
    );
}

/// Whatever is in the collection is well formed. Vacuous today and not for
/// long — this is the check that starts biting the moment a record lands.
#[test]
fn every_record_in_the_collection_is_well_formed() {
    let faults = check(&all(), &read_all(&dir()), &dir());
    assert!(
        faults.is_empty(),
        "{} fault(s):\n{}",
        faults.len(),
        faults
            .iter()
            .map(|f| format!("  {f}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 2 · Every check can fail
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn a_sound_record_passes() {
    assert!(check(&all(), &[sound()], &dir()).is_empty());
}

/// **P0.** A judge who authored the proposition may not rule on it. This is the
/// whole reason for asking someone else, and the one check here that is not
/// bookkeeping.
#[test]
fn a_judge_may_not_rule_on_what_it_authored() {
    let mut r = sound();
    r.judges[0].provenance = vec!["transition-is-a-prism".into()];
    let faults = check(&all(), &[r], &dir());
    assert!(
        matches!(faults.first(), Some(Fault::NonIdentity { judge, .. }) if judge == "a-reader"),
        "{faults:?}"
    );
}

/// The role must be the one the proposition declares. Competence at something
/// else is not competence at this.
#[test]
fn a_record_may_not_substitute_a_different_role() {
    let mut r = sound();
    r.role = "editor".into();
    assert!(matches!(
        check(&all(), &[r], &dir()).first(),
        Some(Fault::WrongRole { .. })
    ));
}

/// A record cannot settle something no test would have needed a judge for.
#[test]
fn a_record_may_not_settle_a_decidable_proposition() {
    let mut r = sound();
    r.proposition = "g3-one-token-one-thread".into();
    assert!(matches!(
        check(&all(), &[r], &dir()).first(),
        Some(Fault::NotJudgmental { .. })
    ));
}

#[test]
fn a_record_may_not_settle_a_proposition_that_does_not_exist() {
    let mut r = sound();
    r.proposition = "no-such-claim".into();
    assert!(matches!(
        check(&all(), &[r], &dir()).first(),
        Some(Fault::Unknown { .. })
    ));
}

/// A verdict with nothing behind it is an assertion, not a judgment. The
/// reasoning is what lets a later reader disagree — which is the only thing
/// that makes a ruling reviewable rather than final.
#[test]
fn a_verdict_needs_an_argument_behind_it() {
    let mut r = sound();
    r.reasoning = String::new();
    assert!(matches!(
        check(&all(), &[r], &dir()).first(),
        Some(Fault::NoReasoning { .. })
    ));
}

#[test]
fn a_record_with_no_judge_settles_nothing() {
    let mut r = sound();
    r.judges.clear();
    assert!(matches!(
        check(&all(), &[r], &dir()).first(),
        Some(Fault::Empty { .. })
    ));
}

#[test]
fn a_judge_without_a_verdict_is_refused() {
    let mut r = sound();
    r.judges[0].verdict = String::new();
    assert!(matches!(
        check(&all(), &[r], &dir()).first(),
        Some(Fault::Empty { .. })
    ));
}

// ════════════════════════════════════════════════════════════════════════════
// 3 · A panel, with nothing deciding its size
// ════════════════════════════════════════════════════════════════════════════

/// **A panel is carried; its size is nobody's business here.**
///
/// `panels` is `⊨` with more than one judge, and the habit in
/// `questions/resolved/_evidence/` is two independent reviews. The schema takes
/// any number and three pass exactly as one does.
///
/// Deciding that a claim *warrants* a deep panel rather than one reasoning
/// model is a judgment about worth, and Het declares no worth law. When HetOpt
/// exists it will find the shape already here — and this test is what would
/// break if a minimum were ever quietly introduced.
#[test]
fn a_panel_of_any_size_is_carried_and_none_is_required() {
    for n in [1usize, 2, 3, 7] {
        let mut r = sound();
        r.judges = (0..n)
            .map(|i| Ruling {
                id: format!("reader-{i}"),
                provenance: vec![format!("elsewhere-{i}")],
                verdict: "conforming".into(),
                epsilon: None,
                on: "2026-08-05".into(),
            })
            .collect();
        assert!(
            check(&all(), &[r], &dir()).is_empty(),
            "a panel of {n} was refused"
        );
    }
}

/// Judges may disagree, and a record carries the disagreement rather than
/// resolving it. Nothing here computes a consensus — that would be a worth law
/// with a majority rule for a face.
#[test]
fn a_split_panel_is_recorded_not_resolved() {
    let mut r = sound();
    r.judges = vec![
        Ruling {
            id: "one".into(),
            provenance: vec!["elsewhere".into()],
            verdict: "conforming".into(),
            epsilon: None,
            on: "2026-08-05".into(),
        },
        Ruling {
            id: "two".into(),
            provenance: vec!["elsewhere".into()],
            verdict: "non-conforming".into(),
            epsilon: None,
            on: "2026-08-05".into(),
        },
    ];
    assert!(check(&all(), &[r.clone()], &dir()).is_empty());
    assert_ne!(r.judges[0].verdict, r.judges[1].verdict);
}

/// A **non-conforming** verdict is a well-formed judgment, not an error.
/// Q7's ruling overturned the account it was asked about, and a collection that
/// only accepted approval would have had nowhere to put it.
#[test]
fn a_ruling_against_the_doctrine_is_a_valid_record() {
    let mut r = sound();
    r.judges[0].verdict = "non-conforming".into();
    r.reasoning = "The backward pass is not a build; the account is wrong here.".into();
    assert!(check(&all(), &[r], &dir()).is_empty());
}

// ════════════════════════════════════════════════════════════════════════════
// 4 · What no record can establish
// ════════════════════════════════════════════════════════════════════════════

/// **An attested record is a receipt.** Nothing distinguishes a faithful
/// transcription from an invention, and the type says so rather than implying
/// otherwise.
///
/// This is the same distinction that separated a `trybuild` case from
/// `(rustc)`: whether the thing establishing a claim can itself fail. A
/// `dispatched` record's provenance came out of a sealed `Judgment`; an
/// attested one's came out of a field somebody typed.
#[test]
fn the_tier_records_whether_a_ruling_can_be_audited() {
    let attested = sound();
    assert_eq!(attested.tier, Tier::Attested);

    let mut dispatched = sound();
    dispatched.tier = Tier::Dispatched;

    // Both are well formed. The checks cannot tell them apart, because the
    // difference is in where the ruling came from and not in what it says.
    assert!(check(&all(), &[attested], &dir()).is_empty());
    assert!(check(&all(), &[dispatched], &dir()).is_empty());
}

/// The doctrine may not name a ruling that is not there.
///
/// Mutation: point a judgmental proposition at a record file and this reddens
/// until the file exists.
#[test]
fn a_named_ruling_must_exist() {
    let mut d = rung_ct::doctrine();
    for e in &mut d.elements {
        if let rung_doctrine::Element::Prop(p) = e
            && p.slug == "transition-is-a-prism"
        {
            p.kind = Kind::Judgmental {
                role: "category-theorist".into(),
                ruling: Some("not-written-yet.md".into()),
            };
        }
    }
    let faults = check(&[d], &[], &dir());
    assert!(
        matches!(faults.first(), Some(Fault::Missing { .. })),
        "{faults:?}"
    );
}
