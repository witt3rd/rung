//! The judgmental verdict space — `verdict-space-with-metric` and
//! `epsilon-reported-with-verdict`.
//!
//! Three things are checked here:
//!
//! 1. **The seal.** A `Weight` has no constructor outside the rendering path
//!    (`Rendering::weighed`), so the confidence a `Judgment` carries is the
//!    one its judge's oracle rendered (trybuild, below).
//! 2. **The metric.** `VerdictPoint::distance` is Het's `d`: symmetric, zero on
//!    itself, non-negative, total variation at most 1, and the triangle
//!    inequality — checked over a deterministic sweep, not a handful of cases.
//! 3. **Well-formedness.** A point outside its space, and a confidence outside
//!    `[0,1]`, are refused rather than carried.
//!
//! The whole-path claim — that two judges of differing confidence report
//! differing ε through `Settled` — is
//! `rung-het/tests/gate_law.rs::two_judges_of_differing_confidence_report_differing_verdicts`.

use rung::{Rendering, SIMPLEX_TOLERANCE, Verdict, VerdictPoint, WeightError};

#[test]
fn a_weight_cannot_be_constructed_outside_the_rendering_path() {
    trybuild::TestCases::new().compile_fail("tests/ui/weight_forged.rs");
}

// ── a deterministic generator, so the sweep needs no new dependency ─────────

struct Lcg(u64);

impl Lcg {
    /// A value in `[0,1]`.
    fn unit(&mut self) -> f64 {
        // Knuth's MMIX constants; the top 53 bits make a uniform f64.
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / ((1u64 << 53) - 1) as f64
    }

    fn probability(&mut self) -> VerdictPoint {
        VerdictPoint::probability(self.unit()).expect("unit() is in [0,1]")
    }

    /// A distribution over `options`, including degenerate (point-mass) ones.
    fn simplex(&mut self, options: &[&str]) -> VerdictPoint {
        let mut raw: Vec<f64> = options.iter().map(|_| self.unit()).collect();
        if self.unit() < 0.2 {
            let hot = (self.unit() * options.len() as f64) as usize % options.len();
            for (i, r) in raw.iter_mut().enumerate() {
                *r = if i == hot { 1.0 } else { 0.0 };
            }
        }
        let total: f64 = raw.iter().sum::<f64>().max(f64::MIN_POSITIVE);
        VerdictPoint::simplex(options.iter().copied().zip(raw.iter().map(|r| r / total)))
            .expect("normalised masses are a distribution")
    }
}

const OPTIONS: &[&str] = &[
    "accept",
    "defer",
    "raises-questions",
    "reject-diagnosis",
    "reject-remedy",
];
const SWEEP: usize = 2_000;
const ROUNDING: f64 = 1e-12;

fn points(g: &mut Lcg) -> Vec<(VerdictPoint, VerdictPoint, VerdictPoint)> {
    (0..SWEEP)
        .map(|i| {
            if i % 2 == 0 {
                (g.probability(), g.probability(), g.probability())
            } else {
                (g.simplex(OPTIONS), g.simplex(OPTIONS), g.simplex(OPTIONS))
            }
        })
        .collect()
}

#[test]
fn distance_is_symmetric() {
    for (a, b, _) in points(&mut Lcg(1)) {
        assert_eq!(a.distance(&b), b.distance(&a), "d({a:?}, {b:?})");
    }
}

#[test]
fn distance_is_zero_on_itself() {
    for (a, _, _) in points(&mut Lcg(2)) {
        assert_eq!(a.distance(&a), Some(0.0), "d({a:?}, itself)");
    }
}

#[test]
fn distance_is_non_negative_and_at_most_one() {
    // For [0,1] that is |p − q| ≤ 1; for Δⁿ it is the total-variation bound.
    for (a, b, _) in points(&mut Lcg(3)) {
        let d = a.distance(&b).expect("same space");
        assert!((0.0..=1.0).contains(&d), "d({a:?}, {b:?}) = {d}");
    }
}

#[test]
fn distance_satisfies_the_triangle_inequality() {
    for (a, b, c) in points(&mut Lcg(4)) {
        let (ab, bc, ac) = (
            a.distance(&b).unwrap(),
            b.distance(&c).unwrap(),
            a.distance(&c).unwrap(),
        );
        assert!(
            ac <= ab + bc + ROUNDING,
            "d(a,c)={ac} > d(a,b)+d(b,c)={}",
            ab + bc
        );
    }
}

/// Disjoint point masses are the far corners of Δⁿ: total variation exactly 1.
#[test]
fn total_variation_reaches_one_on_disjoint_point_masses() {
    let a = VerdictPoint::simplex([("accept", 1.0), ("defer", 0.0)]).unwrap();
    let b = VerdictPoint::simplex([("accept", 0.0), ("defer", 1.0)]).unwrap();
    assert_eq!(a.distance(&b), Some(1.0));
    let p = VerdictPoint::probability(0.25).unwrap();
    let q = VerdictPoint::probability(0.75).unwrap();
    assert_eq!(p.distance(&q), Some(0.5));
}

/// There is no metric across spaces, and none is invented.
#[test]
fn distance_is_undefined_across_spaces() {
    let p = VerdictPoint::probability(0.5).unwrap();
    let s = VerdictPoint::simplex([("yes", 0.5), ("no", 0.5)]).unwrap();
    let t = VerdictPoint::simplex([("yes", 0.5), ("maybe", 0.5)]).unwrap();
    assert_eq!(p.distance(&s), None);
    assert_eq!(s.distance(&p), None);
    assert_eq!(s.distance(&t), None, "different options, different space");
}

// ── well-formedness: what is refused ────────────────────────────────────────

#[test]
fn a_probability_outside_the_unit_interval_is_refused() {
    for p in [-0.01, 1.01, f64::NAN, f64::INFINITY] {
        assert!(
            matches!(
                VerdictPoint::probability(p),
                Err(WeightError::MassOutOfRange(_))
            ),
            "{p}"
        );
    }
}

#[test]
fn a_distribution_that_is_not_one_is_refused() {
    assert_eq!(
        VerdictPoint::simplex(Vec::<(String, f64)>::new()),
        Err(WeightError::EmptySimplex)
    );
    assert_eq!(
        VerdictPoint::simplex([("a", 0.5), ("a", 0.5)]),
        Err(WeightError::RepeatedOption("a".into()))
    );
    assert!(matches!(
        VerdictPoint::simplex([("a", 0.5), ("b", 0.2)]),
        Err(WeightError::NotADistribution(_))
    ));
    assert!(matches!(
        VerdictPoint::simplex([("a", 1.5), ("b", -0.5)]),
        Err(WeightError::MassOutOfRange(_))
    ));
}

/// A judge's rounding is a distribution; it is normalised, not refused.
#[test]
fn a_rounded_distribution_is_accepted_and_normalised() {
    let s = VerdictPoint::simplex([("a", 0.34), ("b", 0.33), ("c", 0.34)]).unwrap();
    let total: f64 = s.as_simplex().unwrap().values().sum();
    assert!((total - 1.0).abs() < ROUNDING);
    assert!(VerdictPoint::simplex([("a", 0.5), ("b", 0.5 + 2.0 * SIMPLEX_TOLERANCE)]).is_err());
}

#[test]
fn a_rendering_refuses_a_confidence_outside_the_unit_interval_and_an_unnamed_source() {
    let point = || VerdictPoint::probability(0.9).unwrap();
    for c in [-0.1, 1.1, f64::NAN] {
        assert!(matches!(
            Rendering::weighed(Verdict::Conforming, point(), c, "judge"),
            Err(WeightError::ConfidenceOutOfRange(_))
        ));
    }
    assert_eq!(
        Rendering::weighed(Verdict::Conforming, point(), 0.9, "  "),
        Err(WeightError::Unattributed)
    );
}

#[test]
fn a_weighed_rendering_reports_its_point_confidence_and_source() {
    let r = Rendering::weighed(
        Verdict::Conforming,
        VerdictPoint::probability(0.9).unwrap(),
        0.75,
        "typesafe/jev-1.13-20260917",
    )
    .unwrap();
    let w = r.weight().expect("weighed");
    assert_eq!(w.point().as_probability(), Some(0.9));
    assert_eq!(w.confidence(), 0.75);
    assert_eq!(w.model(), "typesafe/jev-1.13-20260917");
    assert_eq!(w.epsilon(), 0.25);

    let unweighed: Rendering = Verdict::Conforming.into();
    assert!(
        unweighed.weight().is_none(),
        "a bare verdict is uncalibrated"
    );
}
