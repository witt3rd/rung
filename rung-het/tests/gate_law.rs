//! Conformance tests for `rung-het` — Het's gate law as a type property.
//!
//! Following rung's own discipline (rung-props.md fractal-property): every guarantee names the test
//! that fails if the implementation stops honoring it. The tests that matter
//! most here are the **negative** ones — a gate that never fires on a
//! deliberate violation is not a gate.
//!
//! The compile-fail cases live in `rung-het/src/lib.rs` as doctests, because
//! only a doctest can assert that something does *not* compile.

use rung_het::{
    Consulted, Pool, Principal, Prov, Provenanced, QualifyError, Rendering, Response, Role,
    Verdict, VerdictPoint, theory,
};

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
    /// `(P(M ⊨ φ), confidence)` the oracle reports with its verdict, if any.
    weight: Option<(f64, f64)>,
}

impl Principal for Judge {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }

    /// `authored` — the history this principal claims. `π(p)` is this
    /// **with `id()` added**, by the blanket `Provenanced` impl in `rung`:
    /// the provenance floor is not a value a principal gets to state.
    fn authored(&self) -> Prov {
        Prov::of(self.prov.iter().copied())
    }

    /// The oracle. The verdict is the outside's, not the caller's — and so is
    /// its weight, when it reports one.
    fn rule(&self, _matter: &str) -> Response {
        match self.weight {
            None => Response::Rendered(Verdict::Conforming.into()),
            Some((p, confidence)) => Response::Rendered(
                Rendering::weighed(
                    Verdict::Conforming,
                    VerdictPoint::probability(p).expect("p is in [0,1]"),
                    confidence,
                    self.id,
                )
                .expect("confidence is in [0,1]"),
            ),
        }
    }
}

fn judge(id: &'static str, prov: &[&'static str], roles: &[&'static str]) -> Judge {
    Judge {
        id,
        prov: prov.to_vec(),
        roles: roles.to_vec(),
        weight: None,
    }
}

/// A judge whose oracle reports `P(M ⊨ φ) = p` with `confidence`.
fn weighed_judge(id: &'static str, roles: &[&'static str], p: f64, confidence: f64) -> Judge {
    Judge {
        weight: Some((p, confidence)),
        ..judge(id, &[id], roles)
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
    // The licence and the judgment come from the same principal, in one act:
    // `consult` qualifies and then *asks*. The verdict is never the caller's.
    let (q, judgment) = pool.consult::<ChordReader>(&m, "is_constitutive").unwrap();

    let settled = soul::is_constitutive::settle(&m, q, judgment)
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
    // The outside is a real outside: it can rule against the candidate. Which
    // it does is now the *principal's* doing — `Contrarian::rule` returns
    // non-conforming, and this test has no parameter through which it could
    // have arranged the outcome itself.
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![Contrarian]);
    let (q, judgment) = pool.consult::<ChordReader>(&m, "is_constitutive").unwrap();

    let settled = soul::is_constitutive::settle(&m, q, judgment)
        .expect("the licence was minted against this very argument");
    assert!(!settled.verdict().is_conforming());
}

/// `epsilon-reported-with-verdict` — ε is reported alongside the verdict, as an
/// honest error bar.
///
/// Het's `verdict-space-with-metric` asks for a verdict space carrying a metric
/// `d`, and 4.6 asks that every verdict arrive with its ε. The polarity alone
/// cannot carry it: a judge that is barely persuaded and a judge that is
/// certain return the *same* `Verdict` (`boolean-breaks-satisfaction`).
///
/// So the two settlements below agree on polarity and differ by ε — and the ε
/// each reports is its own judge's, read out of the sealed `Judgment`. Nothing
/// in this test hands a confidence to `settle`; the only place a number enters
/// is each judge's oracle (`Principal::rule`).
///
/// This test was parked (ignored) while `Verdict` was the whole of what a
/// `Settled` carried. It is the gap's own assertion, unparked: if the weight
/// stops reaching the caller — dropped at the seal, dropped at `settle`, or
/// reported as a made-up constant — the two settlements are equal again and
/// this fails.
#[test]
fn two_judges_of_differing_confidence_report_differing_verdicts() {
    let m = doc_by(&["augur"]);
    // Barely persuaded, and certain.
    let pool_a = Pool::new(vec![weighed_judge(
        "forge",
        &[ChordReader::NAME],
        0.55,
        0.1,
    )]);
    let pool_b = Pool::new(vec![weighed_judge(
        "smithy",
        &[ChordReader::NAME],
        0.99,
        0.98,
    )]);

    let (qa, ja) = pool_a
        .consult::<ChordReader>(&m, "is_constitutive")
        .unwrap();
    let a = soul::is_constitutive::settle(&m, qa, ja)
        .expect("the licence was minted against this very argument");

    let (qb, jb) = pool_b
        .consult::<ChordReader>(&m, "is_constitutive")
        .unwrap();
    let b = soul::is_constitutive::settle(&m, qb, jb)
        .expect("the licence was minted against this very argument");

    // The same polarity: the Boolean cannot tell them apart …
    assert_eq!(a.verdict(), b.verdict());
    // … and the ε can, because it reached the caller.
    let (ea, eb) = (
        a.epsilon().expect("forge reported ε"),
        b.epsilon().expect("smithy reported ε"),
    );
    assert!((ea - 0.9).abs() < 1e-12, "forge's ε is 1 − 0.1, got {ea}");
    assert!(
        (eb - 0.02).abs() < 1e-12,
        "smithy's ε is 1 − 0.98, got {eb}"
    );
    assert_ne!(
        a, b,
        "epsilon-reported-with-verdict: two judgmental verdicts of the same \
         polarity are still distinct judgments, and must be told apart by their ε"
    );

    // The point is the judge's too, and the metric measures between the two.
    let (pa, pb) = match (&a, &b) {
        (
            rung_het::Settled::Judgmental { judgment: ja, .. },
            rung_het::Settled::Judgmental { judgment: jb, .. },
        ) => (
            ja.weight().expect("weighed").point().clone(),
            jb.weight().expect("weighed").point().clone(),
        ),
        _ => panic!("both settlements are judgmental"),
    };
    let d = pa.distance(&pb).expect("both are points of [0,1]");
    assert!((d - 0.44).abs() < 1e-12, "d(0.55, 0.99) = 0.44, got {d}");
}

/// The other side of the same proposition: a judge that reports **no** weight
/// is uncalibrated, and the settlement says so — `None`, not a fabricated 0.
/// A decidable sentence is exact, and says that.
#[test]
fn an_unweighed_judge_reports_no_epsilon_and_a_decidable_one_reports_zero() {
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![judge("forge", &["forge"], &[ChordReader::NAME])]);
    let (q, j) = pool.consult::<ChordReader>(&m, "is_constitutive").unwrap();
    assert!(j.weight().is_none());
    let settled = soul::is_constitutive::settle(&m, q, j).unwrap();
    assert_eq!(
        settled.epsilon(),
        None,
        "an uncalibrated judge reports no ε"
    );

    assert_eq!(soul::within_budget::holds(&m).epsilon(), Some(0.0));
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

// ═════════════════════════════════════════════════════════════════════════
// R2 — the outside supplies the verdict (judgment-provenance-is-the-judges)
// ═════════════════════════════════════════════════════════════════════════
//
// Until R2, `settle(model, q, v: Verdict)` took the verdict as a **parameter**
// and no method of `Principal` returned one. So a caller could compute a
// verdict from the model's own carrier and hand it in, and the receipt would
// name a judge that was never asked — `constant-arrow-hazard`, live, on the
// path that carries the shape of a disposition end to end.
//
// Three things are asserted below, and they are the three joints of the chain:
// the oracle is what speaks, the seal is what carries who spoke, and `settle`
// is where the two are required to be the same principal.

/// A principal that always rules against. The point is that *it* decides — the
/// caller of `settle` has no parameter through which to say otherwise.
pub struct Contrarian;

impl Principal for Contrarian {
    fn capable(&self, role_name: &str) -> bool {
        role_name == ChordReader::NAME
    }
    fn id(&self) -> &str {
        "contrarian"
    }
    fn authored(&self) -> Prov {
        Prov::empty()
    }
    fn rule(&self, matter: &str) -> Response {
        Response::Rendered(
            Verdict::NonConforming {
                reason: format!("`{matter}` does not hold, and I am the one asked"),
            }
            .into(),
        )
    }
}

#[test]
fn the_verdict_comes_from_the_oracle_and_not_from_the_caller() {
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![Contrarian]);
    let (q, judgment) = pool
        .consult::<ChordReader>(&m, "is_constitutive")
        .expect("the contrarian is capable and disjoint from augur");

    // Nothing in this test states a verdict. There is no parameter for one.
    let settled = soul::is_constitutive::settle(&m, q, judgment)
        .expect("licence and judgment are the same principal's");

    assert!(
        !settled.verdict().is_conforming(),
        "the settled verdict is the one `Principal::rule` returned; the caller \
         never supplied it and cannot"
    );
}

#[test]
fn a_judgment_rendered_by_another_principal_is_refused() {
    // Both principals are real, both are capable, both are disjoint from the
    // document. Nothing here is forged: the licence was honestly minted for
    // `forge`, and the judgment was honestly rendered by `bellows`. What is
    // wrong is the *pairing* — the receipt would name a judge that did not
    // rule on this, which is the constant arrow wearing a second disguise.
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![judge("forge", &["forge"], &[ChordReader::NAME])]);
    let (q, _forges_own) = pool
        .consult::<ChordReader>(&m, "is_constitutive")
        .expect("forge qualifies");

    let bellows = judge("bellows", &["bellows"], &[ChordReader::NAME]);
    let Consulted::Rendered(borrowed) = bellows.judgment("is_constitutive") else {
        panic!("bellows answers when asked");
    };

    match soul::is_constitutive::settle(&m, q, borrowed) {
        Err(rung_het::SettleError::OutcomeNotFromJudge(e)) => {
            assert_eq!(e.licensed, "forge");
            assert_eq!(e.ruled, "bellows");
        }
        other => panic!("π(f(a)) ⊆ π(p) is asserted where the judgment is spent; got {other:?}"),
    }
}

#[test]
fn a_settled_receipt_carries_the_judges_provenance() {
    // The payoff, and the reason no disjointness epilogue is needed:
    //   π(f(a)) ⊆ π(p)  ∧  π(p) ∩ π(a) = ∅  ⟹  π(f(a)) ∩ π(a) = ∅
    // The first conjunct is what `settle` asserts; the second is what G13's
    // mint already guaranteed. Output admissibility is the conclusion.
    let m = doc_by(&["augur"]);
    let pool = Pool::new(vec![judge("forge", &["forge"], &[ChordReader::NAME])]);
    let (q, judgment) = pool
        .consult::<ChordReader>(&m, "is_constitutive")
        .expect("forge qualifies");

    let judged = judgment.provenance();
    assert!(judged.contains("forge"));
    assert!(
        !judged.overlaps(&m.provenance()),
        "admissibility-subcategories: π(f(a)) ∩ π(a) = ∅, derived rather than \
         checked — the outcome carries π(p), and π(p) ∩ π(a) = ∅ was the \
         condition the licence was minted under"
    );

    let settled = soul::is_constitutive::settle(&m, q, judgment).expect("same principal");
    assert!(settled.consulted_outside());
}
