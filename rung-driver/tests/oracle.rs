//! Reading a model's reply — the one place a ruling could be invented.
//!
//! Everything else in the driver is a comparison of declared facts. This is the
//! only step that turns prose into a verdict, so it is where a lenient reader
//! would quietly manufacture one. Most of what follows checks that it refuses.
//!
//! No network is touched. `read_reply` is pure, which is why it can be tested
//! at all and why the strictness lives there rather than in the transport.

use rung::Verdict;
use rung_driver::oracle_llm::read_reply;
use rung_driver::{Answer, Population};

fn verdict(text: &str) -> Option<Verdict> {
    match read_reply(text) {
        Some(Answer::Verdict(v)) => Some(v),
        _ => None,
    }
}

fn raised(text: &str) -> bool {
    matches!(read_reply(text), Some(Answer::Raised(_)))
}

// ════════════════════════════════════════════════════════════════════════════
// 1 · The three forms
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn the_three_declared_forms_are_read() {
    assert_eq!(verdict("HOLDS"), Some(Verdict::Conforming));

    assert!(matches!(
        verdict("FAILS the slug is not kebab-case"),
        Some(Verdict::NonConforming { reason }) if reason == "the slug is not kebab-case"
    ));

    assert!(raised("CANNOT-SETTLE I would need the cited test"));
}

/// A reply with the answer on the first line and reasoning after it is read —
/// models append explanation, and the instruction is about the first line.
#[test]
fn trailing_prose_after_the_first_line_is_ignored() {
    assert_eq!(
        verdict("HOLDS\n\nBecause the anchor is well formed and the parent resolves."),
        Some(Verdict::Conforming)
    );
}

/// Leading blank lines are tolerated. Nothing else about the shape is.
#[test]
fn leading_whitespace_is_tolerated() {
    assert_eq!(verdict("\n\n  HOLDS  "), Some(Verdict::Conforming));
}

// ════════════════════════════════════════════════════════════════════════════
// 2 · What it refuses to read as a verdict
// ════════════════════════════════════════════════════════════════════════════

/// **The important test.** Anything that is not exactly one of the three forms
/// is unreadable, and unreadable becomes a raised matter upstream — never a
/// verdict.
///
/// Each of these is a real thing a model says, and each would become a ruling
/// under a reader that searched for a keyword instead of matching a form.
#[test]
fn hedging_is_not_a_verdict() {
    for reply in [
        "I think it holds.",
        "This HOLDS, mostly.",
        "Yes",
        "The claim holds.",
        "It appears to hold, though I would want to check the cited test.",
        "HOLDS? Actually, on reflection, FAILS.",
        "**HOLDS**",
        "holds",
        "",
        "   ",
        "Sure! Here's my assessment:\n\nHOLDS",
    ] {
        assert!(
            read_reply(reply).is_none(),
            "read a verdict out of: {reply:?}"
        );
    }
}

/// A model refusing to answer is a **deferral**, not a failure of the claim.
///
/// The distinction is the whole reason the third form exists: reporting
/// `NonConforming` because a judge declined would put a fabricated ruling into
/// the record, and the claim would read as refuted by something nobody judged.
#[test]
fn declining_to_rule_is_not_a_claim_failing() {
    assert!(raised("CANNOT-SETTLE I cannot see the cited test"));
    assert!(verdict("CANNOT-SETTLE I cannot see the cited test").is_none());
}

/// `FAILS` with no reason still carries one, because a non-conforming verdict
/// with an empty reason reads as an assertion with nothing behind it.
#[test]
fn a_bare_failure_still_carries_a_reason() {
    assert!(matches!(
        verdict("FAILS"),
        Some(Verdict::NonConforming { reason }) if !reason.is_empty()
    ));
}

// ════════════════════════════════════════════════════════════════════════════
// 3 · This repository's population
// ════════════════════════════════════════════════════════════════════════════

fn population() -> Population {
    let text = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("docs/population.yaml"),
    )
    .expect("docs/population.yaml");
    Population::from_yaml(&text).expect("the population parses")
}

#[test]
fn the_repositorys_population_parses_and_is_well_formed() {
    let p = population();
    let faults: Vec<_> = p
        .check()
        .into_iter()
        .filter(|e| !matches!(e, rung_driver::ConfigError::Unused { .. }))
        .collect();
    assert!(faults.is_empty(), "{faults:?}");

    assert!(p.role("editor").is_some());
    assert!(p.role("category-theorist").is_some());
    assert!(p.role("maintainer").is_some());
}

/// Judge and author come out as **different principals**, which is the property
/// the whole arrangement rests on.
#[test]
fn the_declared_judges_and_authors_are_disjoint_sets() {
    let p = population();
    let judges: Vec<&str> = p
        .capable_of("editor")
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    let authors: Vec<&str> = p
        .capable_of("maintainer")
        .iter()
        .map(|s| s.id.as_str())
        .collect();

    assert!(judges.contains(&"opus-judge"));
    assert!(authors.contains(&"opus-author"));
    assert!(
        !authors.contains(&"opus-judge"),
        "a declared judge also fills the authoring role"
    );
    assert!(
        !judges.contains(&"opus-author"),
        "a declared author also fills the judging role"
    );
}

/// The author holds standing over the **source**, not over the rendered
/// markdown.
///
/// Writing to a generated file puts a change somewhere the next render
/// silently discards — a successful edit with no effect, which is worse than a
/// refused one.
#[test]
fn the_author_may_write_the_source_and_not_the_rendering() {
    let p = population();
    let author = p.by_id("opus-author").expect("declared");
    assert!(author.standing.iter().any(|s| s == "rung-doctrine/src"));
    assert!(
        !author.standing.iter().any(|s| s.ends_with("-props.md")),
        "the author holds standing over a generated document"
    );
}

/// **The population cannot judge this repository yet, and says so.**
///
/// Every model principal declares an empty `authored`, which would make
/// non-identity hold vacuously against every proposition. That is Q14, open —
/// and this test exists so the placeholder cannot be forgotten and quietly
/// shipped as a working configuration.
///
/// When Q14 is ruled on, this test is what has to change, deliberately.
#[test]
fn the_model_principals_provenance_is_still_a_placeholder() {
    let p = population();
    for id in ["opus-judge", "gpt-judge", "opus-author"] {
        let spec = p.by_id(id).expect("declared");
        assert!(
            spec.authored.is_empty(),
            "{id} now declares provenance — if Q14 is settled, update this test \
             and say what the ruling was"
        );
    }

    // The human's provenance is real, and is what a settled model provenance
    // would have to look like: something that actually disqualifies.
    let human = p.by_id("donald").expect("declared");
    assert!(!human.authored.is_empty());
}
