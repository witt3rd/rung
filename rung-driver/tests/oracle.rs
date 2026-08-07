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
    Answer, Backing, CommissionLog, Oracle, Roster, SystemConfig, Unreachable,
    population_pool_with_log, resolve,
};
use rung_std::principals::Provider;
use std::collections::BTreeMap;
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

/// **Providers are system-wide now; this population declares none inline.**
/// Every model backing omits the provider and resolves through the system
/// DEFAULT, which for rung's own population is OpenRouter.
#[test]
fn a_provider_catalog_may_be_inline() {
    // the system catalog supplies the DEFAULT
    assert_eq!(sys().default(), Some("openrouter"));
    // the real population declares no providers inline (they moved to ~/.rung)
    let p = population();
    assert!(
        p.providers.is_empty(),
        "providers moved to the system catalog"
    );
    // and every model backing omits the provider, so it uses the DEFAULT
    let model_backings: Vec<_> = p
        .principals
        .iter()
        .filter(|s| s.backing.model().is_some())
        .collect();
    assert!(
        model_backings.len() >= 2,
        "the population routes real models through the system-provided endpoint"
    );
    for s in model_backings {
        assert_eq!(
            s.backing.provider(),
            None,
            "backings omit the provider because DEFAULT is OpenRouter"
        );
    }
}

/// **No credential lives in the repository.** The population file carries no
/// secret-shaped value; credentials live in `~/.rung/auth.yaml` (or the real
/// environment), never in a file that gets committed.
#[test]
fn credentials_live_outside_the_repository() {
    let text = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".het/rung-questions/population.yaml"),
    )
    .unwrap();
    for banned in ["api_key:", "apikey", "sk-", "secret:"] {
        assert!(
            !text.to_lowercase().contains(banned),
            "the population carries something shaped like a credential: {banned}"
        );
    }
    // credentials are the system's, not the population's
    let p = population();
    assert!(p.providers.is_empty());
}

/// A missing credential **raises**; it does not refute the claim.
#[test]
fn a_missing_credential_is_unreachable_and_not_a_verdict() {
    let p = population();
    let backing = Backing::Model {
        provider: Some("anthropic".into()),
        model: "claude-opus-4-6".into(),
    };
    match resolve(&p, &backing, &sys()) {
        // anthropic is in the system catalog and its env var is unset here
        Err(Unreachable::NoCredential { provider, env }) => {
            assert_eq!(provider, "anthropic");
            assert_eq!(env, "ANTHROPIC_API_KEY");
        }
        // if a key IS present the resolution succeeds, and that is fine — what
        // must never happen is a verdict coming out of a config fault.
        Ok(config) => {
            assert_eq!(config.base_url, "https://api.anthropic.com/v1");
            assert!(!config.api_key.is_empty());
        }
        Err(other) => panic!("expected a credential fault, got {other}"),
    }
}

/// A backing naming a provider nobody declared is unreachable, and — when the
/// roster uses the inline-override path — `check` reports it before dispatch.
#[test]
fn an_undeclared_provider_is_caught_before_dispatch() {
    let p = population();
    assert!(matches!(
        resolve(
            &p,
            &Backing::Model {
                provider: Some("nowhere".into()),
                model: "m".into()
            },
            &sys()
        ),
        Err(Unreachable::NoSuchProvider(_))
    ));

    // the `check` catch only fires on the inline-override path (a population
    // that declares providers and names one it does not have)
    let mut inline = Roster::from_yaml(
        "providers:\n  - {name: a, base_url: https://x, api_key_env: X}\nroles: []\nprincipals:\n  - {id: q, kind: llm, capabilities: [], backing: {via: model, provider: nowhere, model: m}}\n",
    )
    .unwrap();
    assert!(
        inline
            .check()
            .iter()
            .any(|e| matches!(e, rung_driver::ConfigError::UnknownProvider { .. })),
        "an unreachable principal was not reported on the inline path"
    );
    let _ = &mut inline;
}

/// A principal answering out of band is not served by a model.
#[test]
fn an_out_of_band_principal_is_not_reachable_by_a_model() {
    let p = population();
    assert!(matches!(
        resolve(&p, &Backing::Outside, &sys()),
        Err(Unreachable::NotServedByAModel)
    ));
    assert_eq!(p.by_id("donald").unwrap().backing, Backing::Outside);
}

/// Per-provider settings are honoured, so one endpoint's limits are not
/// silently applied to another's.
#[test]
fn provider_settings_are_per_provider() {
    let s = sys();
    let anthropic = s.provider("anthropic").expect("declared");
    let openrouter = s.provider("openrouter").expect("declared");
    assert_eq!(anthropic.max_tokens, Some(4096));
    assert_eq!(
        openrouter.max_tokens, None,
        "unset means the default, not 4096"
    );
}

/// A system catalog for the tests: the providers the old population declared
/// inline, resolved against, with no credentials in `auth` (so resolve raises
/// `NoCredential` unless the environment supplies one — which is the honest
/// default in CI).
fn sys() -> SystemConfig {
    let providers = vec![
        Provider {
            name: "anthropic".into(),
            base_url: "https://api.anthropic.com/v1".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            timeout_secs: None,
            max_tokens: Some(4096),
            reasoning_level: None,
        },
        Provider {
            name: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            timeout_secs: None,
            max_tokens: Some(4096),
            reasoning_level: None,
        },
        Provider {
            name: "openrouter".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            api_key_env: "OPENROUTER_API_KEY".into(),
            timeout_secs: None,
            max_tokens: None,
            reasoning_level: None,
        },
    ];
    SystemConfig::from_parts(providers, Some("openrouter".into()), BTreeMap::new())
}
