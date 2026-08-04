//! Conformance tests for `rung-het` — Het's gate law as a type property.
//!
//! Following rung's own discipline (rung-props.md fractal-property): every guarantee names the test
//! that fails if the implementation stops honoring it. The tests that matter
//! most here are the **negative** ones — a gate that never fires on a
//! deliberate violation is not a gate.
//!
//! The compile-fail cases live in `rung-het/src/lib.rs` as doctests, because
//! only a doctest can assert that something does *not* compile.

use rung_het::{Pool, Principal, Prov, Provenanced, QualifyError, Role, Verdict, theory};

// ─────────────────────────────────────────────────────────────────────────
// A tiny domain: a constitutive document with a character budget.
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SoulDoc {
    chars: usize,
    authors: Vec<&'static str>,
}

impl Provenanced for SoulDoc {
    fn provenance(&self) -> Prov {
        Prov::of(self.authors.iter().copied())
    }
}

#[derive(Clone, Copy)]
pub struct ChordReader;
impl Role for ChordReader {
    const NAME: &'static str = "chord-reader";
}

#[derive(Clone, Copy)]
pub struct Cartographer;
impl Role for Cartographer {
    const NAME: &'static str = "cartographer";
}

theory!(soul for SoulDoc {
    decidable  within_budget = |m: &SoulDoc| m.chars <= 15_000;
    decidable  has_authors   = |m: &SoulDoc| !m.authors.is_empty();
    judgmental is_constitutive: ChordReader;
});

// ─────────────────────────────────────────────────────────────────────────
// Principals
// ─────────────────────────────────────────────────────────────────────────

pub struct Judge {
    id: &'static str,
    prov: Vec<&'static str>,
    roles: Vec<&'static str>,
}

impl Provenanced for Judge {
    fn provenance(&self) -> Prov {
        Prov::of(self.prov.iter().copied())
    }
}

impl Principal for Judge {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }
}

fn judge(id: &'static str, prov: &[&'static str], roles: &[&'static str]) -> Judge {
    Judge {
        id,
        prov: prov.to_vec(),
        roles: roles.to_vec(),
    }
}

fn doc_by(authors: &[&'static str]) -> SoulDoc {
    SoulDoc {
        chars: 900,
        authors: authors.to_vec(),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// The decidable gate — computed inside, no outside consulted
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn decidable_sentence_settles_without_any_principal() {
    let m = doc_by(&["augur"]);

    // Note the call: no pool, no principal, no token. There is no parameter
    // through which an outside could enter.
    let settled = soul::within_budget::holds(&m);

    assert!(settled.verdict().is_conforming());
    assert!(
        !settled.consulted_outside(),
        "a decidable sentence must not report an outside call"
    );
}

#[test]
fn decidable_sentence_reports_its_own_failure_reason() {
    let m = SoulDoc {
        chars: 20_000,
        authors: vec!["augur"],
    };
    let settled = soul::within_budget::holds(&m);

    match settled.verdict() {
        Verdict::NonConforming { reason } => assert!(reason.contains("within_budget")),
        Verdict::Conforming => panic!("20k chars is over a 15k budget"),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// P0 — the non-identity filter. The load-bearing tests.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn p0_refuses_a_judge_who_authored_the_material() {
    // The failure Het exists to forbid: Augur judging a document Augur wrote.
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![judge("augur", &["augur"], &[ChordReader::NAME])]);

    let err = pool
        .qualify::<ChordReader>(&m)
        .expect_err("a judge sharing provenance with the model must be refused");

    match err {
        QualifyError::NonIdentityViolated { principal, shared } => {
            assert_eq!(principal, "augur");
            assert_eq!(shared, vec!["augur".to_string()]);
        }
        other => panic!("expected NonIdentityViolated, got {other:?}"),
    }
}

#[test]
fn p0_admits_a_judge_with_disjoint_provenance() {
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![judge("forge", &["forge"], &[ChordReader::NAME])]);

    let q = pool
        .qualify::<ChordReader>(&m)
        .expect("a disjoint, capable judge qualifies");

    assert_eq!(q.principal_id(), "forge");
    assert_eq!(q.role_name(), ChordReader::NAME);
}

#[test]
fn p0_refuses_on_partial_overlap_not_only_identity() {
    // Co-authorship is overlap. Disjointness is the condition, not equality.
    let m = doc_by(&["donald", "augur"]);
    let pool = Pool::new(vec![judge(
        "augur",
        &["augur", "elsewhere"],
        &[ChordReader::NAME],
    )]);

    assert!(matches!(
        pool.qualify::<ChordReader>(&m),
        Err(QualifyError::NonIdentityViolated { .. })
    ));
}

#[test]
fn p0_is_not_vacuous_when_the_model_claims_no_author() {
    // The failure mode that reads green: if pi() is empty everywhere,
    // disjointness holds trivially and P0 is enforced in name only.
    let m = doc_by(&[]);
    let pool = Pool::new(vec![judge("anyone", &["anyone"], &[ChordReader::NAME])]);

    assert_eq!(
        pool.qualify::<ChordReader>(&m).unwrap_err(),
        QualifyError::ModelHasNoProvenance,
        "an empty-provenance model must be refused, not trivially admitted"
    );
}

#[test]
fn competence_is_filtered_before_provenance_matters() {
    // Disjoint, but cannot play the role. Both conjuncts of dispatch-is-two-operations are live.
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![judge("forge", &["forge"], &[Cartographer::NAME])]);

    match pool.qualify::<ChordReader>(&m).unwrap_err() {
        QualifyError::NotCapable { principal, role } => {
            assert_eq!(principal, "forge");
            assert_eq!(role, ChordReader::NAME);
        }
        other => panic!("expected NotCapable, got {other:?}"),
    }
}

#[test]
fn qualification_walks_the_pool_and_takes_any_survivor() {
    // Het no-preference-among-judges: dispatch to *a* qualifying judge. Not the best — Het has no
    // worth-law and must not rank. First survivor is Het-correct.
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![
        judge("augur", &["augur"], &[ChordReader::NAME]), // fails P0
        judge("cookie", &["cookie"], &[Cartographer::NAME]), // wrong role
        judge("forge", &["forge"], &[ChordReader::NAME]), // qualifies
    ]);

    let q = pool.qualify::<ChordReader>(&m).expect("forge qualifies");
    assert_eq!(q.principal_id(), "forge");
}

#[test]
fn an_exhausted_pool_reports_exhaustion_not_the_last_failure() {
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![
        judge("augur", &["augur"], &[ChordReader::NAME]),
        judge("cookie", &["cookie"], &[Cartographer::NAME]),
    ]);

    assert_eq!(
        pool.qualify::<ChordReader>(&m).unwrap_err(),
        QualifyError::PoolExhausted { considered: 2 }
    );
}

#[test]
fn an_empty_pool_qualifies_no_one() {
    let m = doc_by(&["augur"]);
    let pool: Pool<Judge> = Pool::new(vec![]);
    assert_eq!(
        pool.qualify::<ChordReader>(&m).unwrap_err(),
        QualifyError::PoolExhausted { considered: 0 }
    );
}

// ─────────────────────────────────────────────────────────────────────────
// The judgmental gate — settled only through a qualified outside
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn judgmental_sentence_records_the_principal_that_settled_it() {
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![judge("forge", &["forge"], &[ChordReader::NAME])]);
    let q = pool.qualify::<ChordReader>(&m).unwrap();

    // The verdict comes from the principal. The crate never fabricates one.
    let settled = soul::is_constitutive::settle(&m, q, Verdict::Conforming)
        .expect("the licence was minted against this very argument");

    assert!(settled.consulted_outside());
    match settled {
        rung_het::Settled::Judgmental {
            sentence,
            role,
            principal,
            ..
        } => {
            assert_eq!(sentence, "is_constitutive");
            assert_eq!(role, ChordReader::NAME);
            assert_eq!(principal, "forge");
        }
        other => panic!("expected a judgmental settlement, got {other:?}"),
    }
}

#[test]
fn a_judgmental_verdict_may_be_non_conforming() {
    // The outside is a real outside: it can rule against the candidate.
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![judge("forge", &["forge"], &[ChordReader::NAME])]);
    let q = pool.qualify::<ChordReader>(&m).unwrap();

    let settled = soul::is_constitutive::settle(
        &m,
        q,
        Verdict::NonConforming {
            reason: "derived, not constitutive".into(),
        },
    )
    .expect("the licence was minted against this very argument");
    assert!(!settled.verdict().is_conforming());
}

/// **PARKED.** `epsilon-reported-with-verdict` — ε is reported alongside the
/// verdict, as an honest error bar. `Verdict` is Boolean, so it is not.
///
/// Het's `verdict-space-with-metric` asks for a verdict space carrying a metric
/// `d`, and 4.6 asks that every verdict arrive with its ε. Under a Boolean
/// verdict space there is nothing to report and nothing to measure: a judge
/// that is barely persuaded and a judge that is certain return the *same
/// value*, and the satisfaction condition does not survive renaming
/// (`boolean-breaks-satisfaction`).
///
/// The two settlements below are exactly that pair. They agree on polarity and
/// on prose, and they are the same object — which is the gap, stated as an
/// assertion rather than as a caveat in a doc comment.
///
/// **Ignored, deliberately.** Nothing here is broken; `Verdict` is Boolean by
/// declaration and says so in its own docs. This is parked so that the day a
/// metric lands, deleting one attribute reports whether ε actually reaches the
/// caller — rather than the gap living only in prose that nothing runs.
#[test]
#[ignore = "GAP: `Verdict` is Boolean (Conforming | NonConforming), so there is \
            no metric d and no ε to report. Closing this needs a verdict space \
            carrying a metric (rung-het-props.md#verdict-space-with-metric) and \
            an ε on `Settled` (rung-het-props.md#epsilon-reported-with-verdict). \
            Unpark by deleting this attribute once `Settled` carries an error \
            bar; the two settlements below must then differ by it."]
fn two_judges_of_differing_confidence_report_differing_verdicts() {
    let m = doc_by(&["augur"]);
    let pool_a = Pool::new(vec![judge("forge", &["forge"], &[ChordReader::NAME])]);
    let pool_b = Pool::new(vec![judge("smithy", &["smithy"], &[ChordReader::NAME])]);

    // Barely persuaded.
    let a = soul::is_constitutive::settle(
        &m,
        pool_a.qualify::<ChordReader>(&m).unwrap(),
        Verdict::NonConforming {
            reason: "derived, not constitutive".into(),
        },
    )
    .expect("the licence was minted against this very argument");

    // Certain.
    let b = soul::is_constitutive::settle(
        &m,
        pool_b.qualify::<ChordReader>(&m).unwrap(),
        Verdict::NonConforming {
            reason: "derived, not constitutive".into(),
        },
    )
    .expect("the licence was minted against this very argument");

    assert_ne!(
        a.verdict(),
        b.verdict(),
        "epsilon-reported-with-verdict: two judgmental verdicts of the same \
         polarity are still distinct judgments, and must be told apart by their \
         ε. Under a Boolean verdict space they cannot be"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Sen(Σ) as data
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn the_theory_exposes_its_sentences_with_their_gates() {
    assert_eq!(
        soul::SENTENCES,
        &[
            ("within_budget", "decidable"),
            ("has_authors", "decidable"),
            ("is_constitutive", "judgmental"),
        ]
    );
}

#[test]
fn every_sentence_carries_a_gate_from_the_declared_vocabulary() {
    // Het gate-marker-required/four-gates: every operation carries an explicit gate marker, and it is one
    // of the declared values. `authorial` and `conditional` are not yet
    // implemented here; when they are, this test must be extended, not relaxed.
    for (name, gate) in soul::SENTENCES {
        assert!(
            matches!(*gate, "decidable" | "judgmental"),
            "sentence `{name}` has unknown gate `{gate}`"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Provenance algebra
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn disjointness_and_containment_are_different_conditions() {
    // The authorial gate is out of scope, but the asymmetry is the point:
    // judgment demands disjointness, authorship demands containment. A pair
    // that satisfies one fails the other.
    let author = Prov::of(["donald", "augur"]);
    let outcome = Prov::of(["augur"]);

    assert!(
        outcome.contained_in(&author),
        "authorial: containment holds"
    );
    assert!(
        outcome.overlaps(&author),
        "judgmental: disjointness fails on the same pair"
    );
}

#[test]
fn empty_provenance_overlaps_nothing() {
    // Stated so the vacuity is visible in a test rather than only in a doc.
    assert!(!Prov::empty().overlaps(&Prov::of(["augur"])));
    assert!(!Prov::of(["augur"]).overlaps(&Prov::empty()));
}
