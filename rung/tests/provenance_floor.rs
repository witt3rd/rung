//! The provenance floor — `π(p) ⊇ {id(p)}`.
//!
//! A principal declaring `π(p) = ∅` is disjoint from everything, so it
//! qualifies to judge every argument in the workspace: a **universal judge**.
//! `Pool::qualify_for` refuses an *argument* with empty provenance
//! (`QualifyError::ModelHasNoProvenance`) precisely because a filter that
//! cannot fail is not a filter — but nothing said the same about the
//! *principal*, and the same vacuity sits on that side of the disjointness.
//!
//! The ruling is that an empty principal provenance must be **underivable**,
//! not refused by a check. So `Principal` no longer requires `Provenanced` as a
//! supertrait and no longer supplies `π` directly. It declares
//! [`Principal::authored`] — the history it *claims*, which MAY be empty — and
//! the only route from a principal to a provenance is the blanket impl
//!
//! ```text
//! impl<P: Principal + ?Sized> Provenanced for P {
//!     fn provenance(&self) -> Prov { self.authored().with(self.id()) }
//! }
//! ```
//!
//! Three facts follow, and this file pins all three. The third — that a
//! hand-written `impl Provenanced for SomePrincipal` is a coherence error — is
//! the structural one: it is what makes the other two properties of the *type
//! system* rather than of a convention every implementor is trusted to keep.

use rung::{Pool, Principal, Prov, Provenanced, QualifyError, Role, Verdict};

#[derive(Clone, Copy)]
struct Reviewer;
impl Role for Reviewer {
    const NAME: &'static str = "reviewer";
}

/// A principal that claims no history whatsoever. Under the old supertrait this
/// was writable — `fn provenance(&self) -> Prov { Prov::empty() }` — and it was
/// the universal judge.
struct Newcomer;

impl Principal for Newcomer {
    fn capable(&self, role_name: &str) -> bool {
        role_name == "reviewer"
    }
    fn id(&self) -> &str {
        "newcomer"
    }
    /// Empty, and permitted to be: a principal with no history in this
    /// repository claims none. The floor is not a demand that it lie.
    fn authored(&self) -> Prov {
        Prov::empty()
    }
    fn rule(&self, _matter: &str) -> Verdict {
        Verdict::Conforming
    }
}

/// A principal that does claim a history.
struct Veteran;

impl Principal for Veteran {
    fn capable(&self, _role_name: &str) -> bool {
        true
    }
    fn id(&self) -> &str {
        "veteran"
    }
    fn authored(&self) -> Prov {
        Prov::of(["folio", "annex"])
    }
    fn rule(&self, _matter: &str) -> Verdict {
        Verdict::Conforming
    }
}

struct Manuscript;
impl Provenanced for Manuscript {
    fn provenance(&self) -> Prov {
        Prov::of(["newcomer"])
    }
}

// ── 1. π(p) always contains id(p) ───────────────────────────────────────────

#[test]
fn a_principals_provenance_always_contains_its_identity() {
    assert!(
        Newcomer.provenance().contains("newcomer"),
        "the floor: π(p) ⊇ {{id(p)}} — a principal is at minimum the author of \
         its own participation"
    );
    assert!(
        Veteran.provenance().contains("veteran"),
        "the floor applies to a principal that already claims a history: the \
         identity is added to what is authored, not substituted for it"
    );
    assert!(
        Veteran.provenance().contains("folio") && Veteran.provenance().contains("annex"),
        "`authored` is preserved; the floor is a union, not a replacement"
    );
}

// ── 2. a principal's provenance is never empty ──────────────────────────────

#[test]
fn a_principal_can_never_present_an_empty_provenance() {
    assert!(
        !Newcomer.provenance().is_empty(),
        "π(p) = ∅ makes p disjoint from everything — a universal judge. There is \
         no longer a term that produces it"
    );
    assert!(!Veteran.provenance().is_empty());
}

// ── 3. the floor bites: the universal judge is gone ─────────────────────────

#[test]
fn the_newcomer_is_no_longer_disjoint_from_its_own_work() {
    // The manuscript is the newcomer's. Under `π(p) = ∅` the newcomer was
    // disjoint from it and would have been admitted to judge its own work; the
    // floor puts `newcomer` into π(p) and the P0 filter refuses.
    let pool = Pool::new(vec![Newcomer]);
    match pool.qualify_for::<Reviewer>(&Manuscript) {
        Err(QualifyError::NonIdentityViolated { principal, shared }) => {
            assert_eq!(principal, "newcomer");
            assert_eq!(shared, vec!["newcomer".to_string()]);
        }
        other => panic!(
            "a principal claiming no history must still not judge its own work; got {other:?}"
        ),
    }
}

// ── 4. the structural enforcement ───────────────────────────────────────────

/// The property the two tests above rest on, stated where it is actually held.
///
/// `Newcomer.provenance()` is not the value `Newcomer` chose to return; it is
/// the value the blanket impl computes, and `Newcomer` has no way to say
/// otherwise. Writing `impl Provenanced for Newcomer` is E0119 — a *coherence*
/// error, refused by the trait solver before any check of ours could run.
///
/// This is the difference between the floor and a guard. A guard on
/// `Pool::qualify` would refuse an empty principal provenance at the point of
/// use, and would be one un-called code path away from vacuous — the exact
/// failure `Qualified`'s seal exists to foreclose. Here there is no code path:
/// the empty provenance is not a value the language will produce.
#[test]
fn a_hand_written_provenanced_impl_for_a_principal_is_a_coherence_error() {
    trybuild::TestCases::new().compile_fail("tests/ui/floor_forged_provenance.rs");
}
