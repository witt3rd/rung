//! RED — the licence is not bound to what it licenses.
//!
//! `Qualified<R>` is unforgeable: no public constructor, sealed field, minted
//! only by [`Pool::qualify`]. That closes *fabrication*. It does not close
//! *transfer*. A token minted honestly against one argument can be spent on a
//! different one, because the token remembers the principal and the role and
//! **forgets the argument it was measured against**.
//!
//! Het's P0 (non-identity-before-dispatch) is not "some disjointness was
//! checked somewhere". It is `π(p) ∩ π(A) = ∅` for the very `A` the operation
//! is applied to — that is what `disjointness-against-argument` says. A token
//! that does not carry `π(A)` cannot discharge that obligation at the point of
//! use, so every consumer is trusting the caller to have minted correctly.
//!
//! Each test below performs the exact act P0 forbids, using only the public
//! API and without touching a private field. They are expected to FAIL once
//! the token is bound.

use rung_het::{
    Authorized, Disposition, Pool, Principal, Proposal, Prov, Provenanced, Role, Steward, Verdict,
    dispose, theory,
};

#[derive(Clone)]
pub struct Doc {
    chars: usize,
    authors: Vec<&'static str>,
}

impl Provenanced for Doc {
    fn provenance(&self) -> Prov {
        Prov::of(self.authors.iter().copied())
    }
}

#[derive(Clone, Copy)]
pub struct Reviewer;
impl Role for Reviewer {
    const NAME: &'static str = "reviewer";
}

theory!(doc for Doc {
    decidable  within_budget  = |m: &Doc| m.chars <= 15_000;
    judgmental is_constitutive: Reviewer;
});

pub struct P {
    id: &'static str,
    prov: Vec<&'static str>,
    roles: Vec<&'static str>,
    standing: Vec<&'static str>,
}

impl Provenanced for P {
    fn provenance(&self) -> Prov {
        Prov::of(self.prov.iter().copied())
    }
}
impl Principal for P {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }
}
impl Steward for P {
    fn has_standing(&self, over: &str) -> bool {
        self.standing.contains(&over)
    }
}

fn principal(id: &'static str, prov: &[&'static str], standing: &[&'static str]) -> P {
    P {
        id,
        prov: prov.to_vec(),
        roles: vec![Reviewer::NAME],
        standing: standing.to_vec(),
    }
}

fn doc_by(authors: &[&'static str]) -> Doc {
    Doc {
        chars: 900,
        authors: authors.to_vec(),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// 1. dispose accepts a token minted against the MODEL
// ─────────────────────────────────────────────────────────────────────────

/// The judge authored the Proposal. `qualify_for(&proposal)` correctly refuses
/// it. But the judge is disjoint from the *model* by construction — that is
/// exactly the observation behind `disjointness-against-argument` — so
/// `qualify(&model)` hands it a token, and `dispose` accepts that token.
///
/// The P0 hole the doctrine closed in prose is still open in the type: the
/// only thing standing between here and self-disposal is the caller choosing
/// the right mint.
#[test]
fn dispose_refuses_a_token_minted_against_the_model() {
    // The model is authored by `donald`. The proposal is authored by `academy`.
    let model = doc_by(&["donald"]);
    let pool = Pool::new(vec![principal("academy", &["academy"], &["cabinet"])]);
    let academy = principal("academy", &["academy"], &["cabinet"]);

    let pen: Authorized<'_> = pool.authorize(&academy, "cabinet").expect("standing");
    let proposal: Proposal<&'static str> = Proposal::remedy(&pen, "obj", "widen-the-budget");

    // Honest route: refused, as it must be.
    assert!(
        pool.qualify_for::<Reviewer>(&proposal).is_err(),
        "academy authored this proposal; qualify_for must refuse it"
    );

    // Dishonest route: the same principal, a token minted against the model.
    let laundered = pool
        .qualify::<Reviewer>(&model)
        .expect("academy is disjoint from the model — that is the whole trap");

    let ruling = dispose(&proposal, laundered, Disposition::Accept);

    assert_ne!(
        ruling.judge(),
        proposal.author(),
        "P0: a judge disposed on a proposal it authored itself, \
         using a token minted against the model"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// 2. settle accepts a token minted against a DIFFERENT model
// ─────────────────────────────────────────────────────────────────────────

/// Two models. `forge` is disjoint from `augur`'s document and qualifies
/// honestly against it. That token then discharges the judgmental sentence on
/// `forge`'s **own** document — the argument the token was never measured
/// against.
///
/// `settle` consumes the token by value, so it cannot be spent twice. But
/// spending it *once, on the wrong model* is unguarded.
#[test]
fn settle_refuses_a_token_minted_against_a_different_model() {
    let augurs = doc_by(&["augur"]);
    let forges = doc_by(&["forge"]);
    let pool = Pool::new(vec![principal("forge", &["forge"], &[])]);

    // Honest route against forge's own doc: refused.
    assert!(
        pool.qualify::<Reviewer>(&forges).is_err(),
        "forge shares provenance with its own doc"
    );

    // Token minted honestly against a *different* model.
    let token = pool
        .qualify::<Reviewer>(&augurs)
        .expect("forge is disjoint from augur's doc");

    let settled = doc::is_constitutive::settle(&forges, token, Verdict::Conforming);

    panic!(
        "P0: forge settled a judgmental sentence on its own document \
         with a licence minted against another: {settled:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// 3. the mirror vacuity — a principal with no provenance
// ─────────────────────────────────────────────────────────────────────────

/// `qualify` refuses a *model* with empty provenance, because every candidate
/// would pass disjointness vacuously and the filter would be ornamental. The
/// mirror vacuity on the *principal's* side is unguarded: a principal
/// declaring `π(p) = ∅` is disjoint from **everything**, forever — a universal
/// judge, admitted by construction against every model in the world.
///
/// Not a transfer bug like the two above; a doctrine gap. Het as written is
/// satisfied by `π(p) = ∅`. The engine invented the model-side guard on its
/// own judgment and did not invent its mirror.
///
/// Note what *is* guarded, and only incidentally: ghost's Proposal inherits
/// ghost's empty provenance, so `qualify_for` refuses it under the
/// **model**-side rule. The proposal path is closed by accident, not by a rule
/// about principals.
#[test]
fn a_principal_with_no_provenance_is_refused() {
    let pool = Pool::new(vec![principal("ghost", &[], &["cabinet"])]);

    for model in [doc_by(&["donald"]), doc_by(&["augur"]), doc_by(&["forge"])] {
        assert!(
            pool.qualify::<Reviewer>(&model).is_err(),
            "a principal with no declared provenance is disjoint from every \
             model; admitting it makes the non-identity filter vacuous"
        );
    }

    // The incidental guard, pinned so that relaxing the model-side rule cannot
    // silently open the proposal path too.
    let ghost = principal("ghost", &[], &["cabinet"]);
    let pen: Authorized<'_> = pool.authorize(&ghost, "cabinet").expect("standing");
    let proposal: Proposal<&'static str> = Proposal::remedy(&pen, "obj", "edit");
    assert!(pool.qualify_for::<Reviewer>(&proposal).is_err());
}
