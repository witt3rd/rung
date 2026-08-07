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
use rung_driver::{
    Answer, Backing, CommissionLog, Oracle, Roster, Unreachable, population_pool_with_log, resolve,
};
use std::sync::Arc;

struct Answering;
impl Oracle for Answering {
    fn ask(&self, _id: &str, _backing: &Backing, _matter: &str) -> Answer {
        Answer::holds()
    }
}

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

fn population() -> Roster {
    let text = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".het/rung-questions/population.yaml"),
    )
    .expect("population.yaml");
    Roster::from_yaml(&text).expect("the population parses")
}

fn commissions() -> CommissionLog {
    let text = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".het/rung-questions/commissions.yaml"),
    )
    .expect("commissions.yaml");
    CommissionLog::from_yaml(&text).expect("the commission record parses")
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

    assert!(judges.contains(&"gpt-judge"));
    assert!(authors.contains(&"ds-maintainer"));
    assert!(
        !authors.contains(&"gpt-judge"),
        "a declared judge also fills the authoring role"
    );
    assert!(
        !judges.contains(&"ds-maintainer"),
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
    let author = p.by_id("ds-maintainer").expect("declared");
    assert!(author.stewards.iter().any(|s| s == "rung-doctrine/src"));
    assert!(
        !author.stewards.iter().any(|s| s.ends_with("-props.md")),
        "the author holds standing over a generated document"
    );
}

/// **Model provenance is now derived, not declared.**
///
/// Q14 ruled the map, Q16 ruled the carrier (a commission contribution
/// record), and Q17 built the wiring: a model declares a stable `family`, and
/// the pool derives `authored(p)` from `commissions.yaml` by looking that
/// family up at qualification time. A model must NOT carry a static `authored`
/// list — that is the growing second source of truth the carrier exists to
/// remove, and the driver refuses it (`FamilyWithAuthored`).
#[test]
fn model_provenance_is_derived_from_the_commission_record() {
    let p = population();
    let log = commissions();
    for id in [
        "opus-theorist",
        "gpt-judge",
        "grok-theorist",
        "ds-maintainer",
        "ds-curator",
        "gpt-interrogator",
        "gpt-adjudicator",
    ] {
        let spec = p.by_id(id).expect("declared");
        assert!(
            spec.family.is_some(),
            "{id} must declare a `family` so its provenance is derived"
        );
        assert!(
            spec.provenance.is_empty(),
            "{id} must not carry a static `authored` list — provenance is derived"
        );
        // The artificial family coincides with its backing model: the pool can
        // actually look it up. Whether it is yet non-empty is the record's
        // state, not the mechanism's — see the commission tests.
        let _ = log.artifacts_for(spec.family.as_deref().unwrap());
    }

    // Every model principal derives through a wired log without a fault.
    let _ = population_pool_with_log(&p, "editor", Arc::new(Answering), Arc::new(log));

    // The human's provenance is real, and is what a *declared* record looks
    // like: something that actually disqualifies.
    let human = p.by_id("donald").expect("declared");
    assert!(human.family.is_none());
    assert!(!human.provenance.is_empty());
}

// ════════════════════════════════════════════════════════════════════════════
// 4 · Providers — many endpoints, and no secret in the file
// ════════════════════════════════════════════════════════════════════════════

/// Several providers, and each principal resolves to its own.
#[test]
fn each_principal_resolves_to_the_provider_that_serves_it() {
    let p = population();
    assert!(
        p.providers.len() >= 2,
        "a population spanning one provider proves nothing"
    );

    let judge = p.by_id("gpt-judge").unwrap();
    let other = p.by_id("opus-theorist").unwrap();
    assert_ne!(
        judge.backing.provider(),
        other.backing.provider(),
        "the two judges share an endpoint; a second family was the point"
    );
}

/// **No credential is declared anywhere.** The population names environment
/// variables; a schema that could hold a key would eventually hold one, and
/// this file is in the repository.
#[test]
fn the_population_names_credentials_and_never_holds_one() {
    let text = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".het/rung-questions/population.yaml"),
    )
    .unwrap();

    // Round-tripping proves the schema, not just this file: if `Provider` ever
    // gained a literal-secret field, serializing would surface it here.
    let round = serde_yaml::to_string(&population()).unwrap();
    for hay in [text.as_str(), round.as_str()] {
        for banned in ["api_key:", "apikey", "sk-", "secret:"] {
            assert!(
                !hay.to_lowercase().contains(banned),
                "the population carries something shaped like a credential: {banned}"
            );
        }
    }
    for pr in &population().providers {
        assert!(!pr.api_key_env.is_empty(), "{} names no variable", pr.name);
    }
}

/// A missing credential **raises**; it does not refute the claim.
///
/// The distinction is the whole reason `Unreachable` exists: a claim reported
/// as `NonConforming` because an environment variable was unset would be
/// refuted by a configuration mistake, and nothing would say so.
#[test]
fn a_missing_credential_is_unreachable_and_not_a_verdict() {
    let p = population();
    let backing = Backing::Model {
        provider: "anthropic".into(),
        model: "claude-opus-4-6".into(),
    };

    // The variable this population names is almost certainly unset in CI, and
    // that must not read as a judgment either way.
    match resolve(&p, &backing) {
        Err(Unreachable::NoCredential { provider, env }) => {
            assert_eq!(provider, "anthropic");
            assert_eq!(env, "ANTHROPIC_API_KEY");
        }
        // If a key IS present the resolution succeeds, and that is fine — what
        // must never happen is a verdict coming out of a config fault.
        Ok(config) => {
            assert_eq!(config.base_url, "https://api.anthropic.com/v1");
            assert!(!config.api_key.is_empty());
        }
        Err(other) => panic!("expected a credential fault, got {other}"),
    }
}

/// A backing naming a provider nobody declared is unreachable, and `check`
/// reports it before any dispatch is attempted.
#[test]
fn an_undeclared_provider_is_caught_before_dispatch() {
    let mut p = population();
    assert!(matches!(
        resolve(
            &p,
            &Backing::Model {
                provider: "nowhere".into(),
                model: "m".into()
            }
        ),
        Err(Unreachable::NoSuchProvider(_))
    ));

    p.principals[1].backing = Backing::Model {
        provider: "nowhere".into(),
        model: "m".into(),
    };
    assert!(
        p.check()
            .iter()
            .any(|e| matches!(e, rung_driver::ConfigError::UnknownProvider { .. })),
        "an unreachable principal was not reported"
    );
}

/// A principal answering out of band is not served by a model, and saying so is
/// not an error — it is the human, and this oracle is not the route to them.
#[test]
fn an_out_of_band_principal_is_not_reachable_by_a_model() {
    let p = population();
    assert!(matches!(
        resolve(&p, &Backing::Outside),
        Err(Unreachable::NotServedByAModel)
    ));
    assert_eq!(p.by_id("donald").unwrap().backing, Backing::Outside);
}

/// Per-provider settings are honoured, so one endpoint's limits are not
/// silently applied to another's.
#[test]
fn provider_settings_are_per_provider() {
    let p = population();
    let anthropic = p.provider("anthropic").expect("declared");
    let openrouter = p.provider("openrouter").expect("declared");
    assert_eq!(anthropic.max_tokens, Some(4096));
    assert_eq!(
        openrouter.max_tokens, None,
        "unset means the default, not 4096"
    );
}
