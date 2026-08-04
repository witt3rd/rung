//! rung-het — Het's gate-marked satisfaction, enforced by the type system.
//!
//! ## What this is
//!
//! Het (see `witt3rd/heteronomy`, here as `docs/rung-het-propositions.md`)
//! extends institution theory at exactly one point: the satisfaction relation `M ⊨ φ`. Every
//! sentence carries a **gate marker** fixing *how* satisfaction is computed:
//!
//! | gate | how `M ⊨ φ` is settled |
//! |---|---|
//! | `decidable` | machine-checked. A pure function of the model. |
//! | `judgmental` | dispatched to a **principal** — an outside. Its verdict *is* the outcome. |
//!
//! This crate makes that distinction a property of the **type**, not of a
//! convention. The load-bearing claim:
//!
//! > A decidable sentence **cannot** consult an outside, because the capability
//! > to do so is not in scope for its body. A judgmental sentence **cannot**
//! > return without one, because its body must consume a [`Qualified`] token
//! > that only the non-identity filter can mint.
//!
//! Het calls that second property **P0**: nothing self-certifies. Here it is a
//! compile error.
//!
//! ## Why this belongs in rung
//!
//! `rung-ct-propositions.md` states the law rung already enforces (`the-law`):
//! *a verb can only live
//! on a morphism, never inside an object* — enforced by sealed constructors
//! (SPEC.md G2). Het's gate law is the same move on a second axis: **an outside
//! call can only live on a judgmental arrow.** Same mechanism — seal the
//! capability, hand it only to the arrow licensed to hold it.
//!
//! ## The shape
//!
//! ```rust
//! use rung_het::{theory, Prov, Provenanced, Role};
//!
//! pub struct SoulDoc { chars: usize }
//! impl Provenanced for SoulDoc {
//!     fn provenance(&self) -> Prov { Prov::of(["augur"]) }
//! }
//!
//! #[derive(Clone, Copy)]
//! pub struct ChordReader;
//! impl Role for ChordReader { const NAME: &'static str = "chord-reader"; }
//!
//! theory!(soul for SoulDoc {
//!     decidable  within_budget   = |m: &SoulDoc| m.chars <= 15_000;
//!     judgmental is_constitutive: ChordReader;
//! });
//!
//! # fn main() {
//! assert_eq!(soul::SENTENCES, &[
//!     ("within_budget", "decidable"),
//!     ("is_constitutive", "judgmental"),
//! ]);
//! # }
//! ```
//!
//! `within_budget`'s body is a plain predicate — it has no pool, no principal,
//! no way to reach outside. `is_constitutive` declares the **competence role**
//! it needs (Het judgmental-declares-role) and can only be discharged by a principal that filter
//! [`Pool::qualify`] admitted.
//!
//! ## Scope
//!
//! ## The gate law, as compile errors
//!
//! rung's own discipline (SPEC.md fractal-property): a guarantee that no test can break is
//! not a guarantee. Each case below is a `compile_fail` doctest — if the
//! enforcement is ever weakened, the example starts compiling and the test
//! fails.
//!
//! ### A decidable body cannot reach an outside
//!
//! Het gate-faithful (gate-faithfulness): a `decidable` operation must factor through
//! `η` — pure, internal, no principal consulted. Here that is not a rule the
//! body is asked to respect; `holds` takes only the model, so there is no
//! parameter through which a pool or a token could arrive.
//!
//! ```compile_fail
//! use rung_het::{theory, Pool, Principal, Prov, Provenanced, Role};
//! pub struct Doc { chars: usize }
//! impl Provenanced for Doc { fn provenance(&self) -> Prov { Prov::of(["a"]) } }
//! pub struct J;
//! impl Provenanced for J { fn provenance(&self) -> Prov { Prov::of(["b"]) } }
//! impl Principal for J {
//!     fn capable(&self, _: &str) -> bool { true }
//!     fn id(&self) -> &str { "j" }
//! }
//! theory!(t for Doc {
//!     decidable within_budget = |m: &Doc| m.chars <= 100;
//! });
//! # fn main() {}
//! // The pool is well-formed; the ONLY error is that `holds` has arity 1 and
//! // so cannot be handed an outside. E0061, and nothing else.
//! fn decidable_cannot_consult(m: &Doc, pool: &Pool<J>) {
//!     let _ = t::within_budget::holds(m, pool);
//! }
//! ```
//!
//! ### A judgmental sentence cannot be settled without a qualified outside
//!
//! Het non-identity-before-dispatch/non-identity-not-deferrable — **P0**. `settle` consumes a [`Qualified`], and [`Qualified`] has
//! no public constructor: [`Pool::qualify`] is the only mint, and it runs the
//! non-identity filter. So there is no term for "settle this judgmental
//! sentence without consulting anyone."
//!
//! ```compile_fail
//! use rung_het::{theory, Prov, Provenanced, Role, Verdict};
//! # #[derive(Clone)] struct Doc;
//! # impl Provenanced for Doc { fn provenance(&self) -> Prov { Prov::of(["a"]) } }
//! # #[derive(Clone, Copy)] struct ChordReader;
//! # impl Role for ChordReader { const NAME: &'static str = "chord-reader"; }
//! theory!(t for Doc {
//!     judgmental is_constitutive: ChordReader;
//! });
//! fn settle_without_an_outside(m: &Doc) {
//!     // `settle` requires a Qualified<ChordReader>; none can be constructed here.
//!     let _ = t::is_constitutive::settle(m, Verdict::Conforming);
//! }
//! # fn main() {}
//! ```
//!
//! ### A `Qualified` token cannot be fabricated
//!
//! The seal (rung SPEC.md G2, applied to the capability rather than the rung).
//! If this compiled, P0 would be a convention.
//!
//! ```compile_fail
//! use rung_het::{Qualified, Role};
//! #[derive(Clone, Copy)]
//! pub struct R;
//! impl Role for R { const NAME: &'static str = "r"; }
//! fn forge_a_licence() -> Qualified<R> {
//!     Qualified { _seal: (), _not_send: std::marker::PhantomData,
//!                 principal_id: "me".into(), principal_prov: Default::default(),
//!                 _role: std::marker::PhantomData }
//! }
//! fn main() { let _ = forge_a_licence(); }
//! ```
//!
//! ### A licence is spent on one sentence
//!
//! `settle` takes the token **by value**. Qualifying once does not license a
//! second judgment — each dispatch runs the filter again, because the model
//! being judged may differ.
//!
//! ```compile_fail
//! use rung_het::{theory, Pool, Principal, Prov, Provenanced, Role, Verdict};
//! # #[derive(Clone)] struct Doc;
//! # impl Provenanced for Doc { fn provenance(&self) -> Prov { Prov::of(["a"]) } }
//! # #[derive(Clone, Copy)] struct R;
//! # impl Role for R { const NAME: &'static str = "r"; }
//! # struct J;
//! # impl Provenanced for J { fn provenance(&self) -> Prov { Prov::of(["b"]) } }
//! # impl Principal for J { fn capable(&self, _: &str) -> bool { true } fn id(&self) -> &str { "j" } }
//! theory!(t for Doc {
//!     judgmental first: R;
//!     judgmental second: R;
//! });
//! fn reuse(m: &Doc, pool: &Pool<J>) {
//!     let q = pool.qualify::<R>(m).unwrap();
//!     let _ = t::first::settle(m, q, Verdict::Conforming);
//!     let _ = t::second::settle(m, q, Verdict::Conforming); // q was moved
//! }
//! # fn main() {}
//! ```
//!
//! ### A judgmental sentence must declare its role
//!
//! Het judgmental-declares-role. This is the gap that went unnoticed in a prose encoding — the map
//! from sentences to roles was simply absent, and nothing could notice.
//! Here it is a parse error with an explanation.
//!
//! ```compile_fail
//! use rung_het::{theory, Prov, Provenanced};
//! # #[derive(Clone)] struct Doc;
//! # impl Provenanced for Doc { fn provenance(&self) -> Prov { Prov::of(["a"]) } }
//! theory!(t for Doc {
//!     judgmental is_constitutive;   // no role — refused
//! });
//! # fn main() {}
//! ```
//!
//! ### A judgmental sentence cannot carry a body
//!
//! If it can be computed it is decidable; marking it judgmental would launder
//! a machine check into an outside call, which is the gate law (no-laundering-along-morphisms) read at
//! the declaration site.
//!
//! ```compile_fail
//! use rung_het::{theory, Prov, Provenanced};
//! # #[derive(Clone)] struct Doc { n: usize }
//! # impl Provenanced for Doc { fn provenance(&self) -> Prov { Prov::of(["a"]) } }
//! theory!(t for Doc {
//!     judgmental is_constitutive = |m: &Doc| m.n > 0;   // refused
//! });
//! # fn main() {}
//! ```
//!
//! Deliberately partial. Implemented: the `decidable` and `judgmental` gates,
//! the non-identity filter, `role(φ)`. Not implemented: the `authorial` gate
//! (standing rather than disjointness), the `conditional` gate (classified one
//! level up, and the first place Het has not decided what the encoding needs),
//! the verdict metric `d`, and `ε`. See `docs/HET-GATES.md`.

use std::collections::BTreeSet;
use std::marker::PhantomData;

// ─────────────────────────────────────────────────────────────────────────
// Provenance
// ─────────────────────────────────────────────────────────────────────────

/// A provenance tag set — Het's `π(x)`, the authorship of a thing.
///
/// Het provenance-structure: the base category carries a provenance map `π_X: X → Prov` into a
/// discrete category of tags. Both gate filters read it. Concretely a finite
/// set of identifiers, and disjointness is decidable over finite sets (non-identity-before-dispatch).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prov(BTreeSet<String>);

impl Prov {
    /// The empty provenance. **Read the caveat.**
    ///
    /// An object with empty provenance is disjoint from *everything*, so every
    /// principal qualifies to judge it. That is not a loophole in the filter —
    /// it is the honest consequence of claiming a thing has no author. But it
    /// is the shape a vacuous P0 takes: if `π` returns empty everywhere,
    /// disjointness passes trivially and the filter is enforced in name only.
    ///
    /// [`Pool::qualify`] therefore **refuses** to admit anyone against a model
    /// whose provenance is empty; see [`QualifyError::ModelHasNoProvenance`].
    pub fn empty() -> Self {
        Self(BTreeSet::new())
    }

    /// Provenance from a set of author tags.
    pub fn of<I, S>(tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self(tags.into_iter().map(Into::into).collect())
    }

    /// Whether any tag is shared. Het's `π(p) ∩ π(M) ≠ ∅`.
    pub fn overlaps(&self, other: &Prov) -> bool {
        self.0.intersection(&other.0).next().is_some()
    }

    /// Whether every tag of `self` is also in `other` — `π(self) ⊆ π(other)`.
    ///
    /// The *authorial* condition (Het admissibility-subcategories). Unused here: the authorial gate is
    /// out of scope for this crate. Present because the asymmetry is the point
    /// — judgment demands disjointness, authorship demands containment — and
    /// omitting the second makes the first look like the only option.
    pub fn contained_in(&self, other: &Prov) -> bool {
        self.0.is_subset(&other.0)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A thing that carries provenance. Objects and principals both do.
pub trait Provenanced {
    fn provenance(&self) -> Prov;
}

// ─────────────────────────────────────────────────────────────────────────
// Roles — Het judgmental-declares-role / capable-single-arity
// ─────────────────────────────────────────────────────────────────────────

/// A **competence role** — what a judgmental sentence needs done.
///
/// Het judgmental-declares-role: every judgmental operation MUST declare the competence role required
/// to discharge it. capable-single-arity pins `capable` to exactly one arity — `𝒫 × Role` — so
/// a principal is asked *"can you play this role?"*, never *"can you judge this
/// sentence?"*. A principal does not have the theory's sentences and cannot be
/// asked to inspect them.
///
/// Making `Role` a type is what supplies `role(φ)`. In a prose encoding the map
/// from sentences to roles can simply be missing — it was, in heteronomy's YAML
/// encoding, and went unnoticed until the interface was written down. Here a
/// judgmental sentence without a role is not a term.
pub trait Role: Copy + 'static {
    const NAME: &'static str;
}

/// A judgmental sentence, and the role it requires.
///
/// This trait **is** `role(φ)` — Het's map from a sentence to the competence
/// needed to discharge it (judgmental-declares-role). Implemented automatically by `theory!` for
/// every judgmental sentence, and impossible to implement without naming a
/// role, since `Requires` is an associated type with no default.
///
/// The gap this closes: in a prose encoding, a judgmental sentence that
/// declared no role was simply a sentence with a field missing, and nothing
/// could notice. Here a judgmental sentence that names no role does not
/// typecheck — there is no term for it.
pub trait Judgmental {
    /// The competence role a principal must be capable of to settle this.
    type Requires: Role;

    /// The sentence's name, for the receipt.
    const SENTENCE: &'static str;

    /// `role(φ)` — the role name, resolved through the type.
    fn role_name() -> &'static str {
        <Self::Requires as Role>::NAME
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Principals — Het unmarked-not-wellformed, the interface
// ─────────────────────────────────────────────────────────────────────────

/// An inhabitant of the pool `𝒫` — a concrete outside.
///
/// Het nothing-further-required: the theory requires exactly four predicates of a principal
/// supplier and **nothing further**. No kinds, no substrates, no identity
/// fields. `capable` and `π` are here; `standing` belongs to the authorial gate
/// (out of scope) and `ε` to the verdict metric (out of scope).
pub trait Principal: Provenanced {
    /// Het's `capable(p, Role)`, at its one arity (capable-single-arity).
    fn capable(&self, role_name: &str) -> bool;

    /// A human-readable identity, for the receipt. Not read by any filter.
    fn id(&self) -> &str;
}

/// Why a principal was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum QualifyError {
    /// `capable(p, role) = false` — competence filter.
    NotCapable {
        principal: String,
        role: &'static str,
    },
    /// `π(p) ∩ π(M) ≠ ∅` — **P0**. The judge authored the material.
    NonIdentityViolated {
        principal: String,
        shared: Vec<String>,
    },
    /// The model claims no author, so disjointness would hold vacuously.
    ///
    /// Refused rather than admitted: a filter that cannot fail is not a filter,
    /// and this is the exact shape in which P0 becomes decorative.
    ModelHasNoProvenance,
    /// No principal in the pool survived both filters.
    PoolExhausted { considered: usize },
}

impl std::fmt::Display for QualifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotCapable { principal, role } => {
                write!(f, "{principal} is not capable of role `{role}`")
            }
            Self::NonIdentityViolated { principal, shared } => write!(
                f,
                "P0: {principal} shares provenance {shared:?} with the model under judgment"
            ),
            Self::ModelHasNoProvenance => write!(
                f,
                "model declares no provenance; disjointness would hold vacuously and P0 would be decorative"
            ),
            Self::PoolExhausted { considered } => {
                write!(f, "no qualifying principal among {considered} considered")
            }
        }
    }
}

impl std::error::Error for QualifyError {}

// ─────────────────────────────────────────────────────────────────────────
// Qualified — the sealed capability
// ─────────────────────────────────────────────────────────────────────────

/// Proof that a principal qualified to judge **this** model under role `R`.
///
/// This is the crate's load-bearing type. It has no public constructor: the
/// only way to obtain one is [`Pool::qualify`], which runs the competence
/// filter and the non-identity filter and refuses on either. Sealed exactly as
/// rung seals its rungs (SPEC.md G2), and for the same categorical reason — a
/// capability that could be fabricated in object-position is not a capability.
///
/// A judgmental sentence's body must **consume** one to return. So:
///
/// - a judgmental sentence cannot be discharged without an outside;
/// - the outside cannot be the author of what it judges;
/// - and neither fact depends on anyone remembering to check.
///
/// That is Het non-identity-before-dispatch/non-identity-not-deferrable — P0 — as a property of the type rather than of a code
/// path. The failure mode this forecloses is a real one: an implementation may
/// have a correct, well-tested qualification filter that nothing on the
/// dispatch path ever calls.
#[must_use = "a Qualified token is a licence to judge; dropping it discards the outside"]
pub struct Qualified<R: Role> {
    _seal: (),
    _not_send: PhantomData<*const ()>,
    principal_id: String,
    principal_prov: Prov,
    _role: PhantomData<R>,
}

impl<R: Role> Qualified<R> {
    /// The identity of the qualifying principal — for the receipt.
    pub fn principal_id(&self) -> &str {
        &self.principal_id
    }

    /// The qualifying principal's provenance, so a verdict can carry it.
    pub fn principal_provenance(&self) -> &Prov {
        &self.principal_prov
    }

    pub fn role_name(&self) -> &'static str {
        R::NAME
    }
}

impl<R: Role> std::fmt::Debug for Qualified<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Qualified<{}>({})", R::NAME, self.principal_id)
    }
}

// ─────────────────────────────────────────────────────────────────────────
// The pool
// ─────────────────────────────────────────────────────────────────────────

/// The principal pool `𝒫`, and the only mint for [`Qualified`].
pub struct Pool<P: Principal> {
    principals: Vec<P>,
}

impl<P: Principal> Pool<P> {
    pub fn new(principals: Vec<P>) -> Self {
        Self { principals }
    }

    pub fn len(&self) -> usize {
        self.principals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.principals.is_empty()
    }

    /// Het dispatch-is-two-operations — the qualifying set, then *any* member of it.
    ///
    /// ```text
    /// qualifying = { p ∈ 𝒫 : capable(p, role(φ)) ∧ π(p) ∩ π(M) = ∅ }
    /// return any(qualifying)
    /// ```
    ///
    /// **Any**, not the best. Het no-preference-among-judges forbids tiering, costing, or preferring
    /// among qualifying judges — ordering is HetOpt's, and Het has no worth-law.
    /// Returning the first survivor is not a hardcode deferred; it is
    /// Het-correct. `argmin` is the named seam where HetOpt would land.
    ///
    /// Both conjuncts read only the declared interface, so this is decidable
    /// and testable cold — no outside call is made to decide who may be called.
    pub fn qualify<R: Role>(&self, model: &dyn Provenanced) -> Result<Qualified<R>, QualifyError> {
        let model_prov = model.provenance();

        // Refuse before filtering. With empty model provenance every candidate
        // passes disjointness and the filter becomes ornamental.
        if model_prov.is_empty() {
            return Err(QualifyError::ModelHasNoProvenance);
        }

        let mut last: Option<QualifyError> = None;

        for p in &self.principals {
            if !p.capable(R::NAME) {
                last = Some(QualifyError::NotCapable {
                    principal: p.id().to_string(),
                    role: R::NAME,
                });
                continue;
            }

            let p_prov = p.provenance();
            if p_prov.overlaps(&model_prov) {
                let shared: Vec<String> = p_prov.0.intersection(&model_prov.0).cloned().collect();
                last = Some(QualifyError::NonIdentityViolated {
                    principal: p.id().to_string(),
                    shared,
                });
                continue;
            }

            return Ok(Qualified {
                _seal: (),
                _not_send: PhantomData,
                principal_id: p.id().to_string(),
                principal_prov: p_prov,
                _role: PhantomData,
            });
        }

        // One candidate: report why it failed. Several: report exhaustion, since
        // naming only the last is misleading.
        Err(match (self.principals.len(), last) {
            (1, Some(e)) => e,
            (n, _) => QualifyError::PoolExhausted { considered: n },
        })
    }
    /// Het dispatch-is-two-operations applied to an arbitrary **argument** (disjointness-against-argument).
    ///
    /// [`Pool::qualify`] measures disjointness against a model; this measures
    /// it against whatever the operation is applied to. At `audit` those are
    /// the same object. At `dispose` they are not — the argument is a Proposal,
    /// whose provenance is its author's (proposal-provenance-is-authors).
    ///
    /// That difference is a live P0 hole when it is missed: a judge that
    /// authored a Proposal is disjoint from the *model* by construction, so a
    /// model-relative check admits it to rule on its own work.
    pub fn qualify_for<R: Role>(
        &self,
        argument: &dyn Provenanced,
    ) -> Result<Qualified<R>, QualifyError> {
        self.qualify::<R>(argument)
    }

    /// How standing is settled for this principal over this container.
    ///
    /// standing-conditional-gated — standing is the one **conditional** gate Het settles. Provenance
    /// containment decides it where it applies; otherwise a judge must rule
    /// (standing-terminates-at-depth-one: terminating at depth one, that judge's own qualification being
    /// plain non-identity relative to the **author**).
    ///
    /// The classifier is itself decidable, as classifier-not-judgmental requires: asking *"does
    /// containment settle this?"* is structural inspection, not judgment. A
    /// judgmental classifier would reopen the regress constant-arrow-hazard closes.
    pub fn classify_standing<S: Steward>(&self, principal: &S, over: &str) -> StandingGate {
        if principal.has_standing(over) {
            StandingGate::Decidable
        } else {
            StandingGate::Judgmental
        }
    }

    /// Mint an [`Authorized`] pen — the authorial filter.
    ///
    /// ```text
    /// P_auth(o, M) = { p ∈ P : capable(p, role(o)) ∧ standing(p, M) }
    /// ```
    ///
    /// The mirror of [`Pool::qualify`]: that one demands the principal did
    /// **not** author the argument; this one demands the object is theirs to
    /// revise. Judgment refuses the audited party; authorship requires standing
    /// over it (authorial-declares-standing).
    ///
    /// Refuses on the judgmental branch rather than guessing. When containment
    /// does not settle standing, Het says a judge must rule on it — and this
    /// engine cannot invent that ruling. Surfacing
    /// [`AuthorizeError::StandingIsJudgmental`] is the honest outcome; closing
    /// it requires the outside.
    pub fn authorize<'a, S: Steward>(
        &self,
        principal: &'a S,
        over: &'a str,
    ) -> Result<Authorized<'a>, AuthorizeError> {
        match self.classify_standing(principal, over) {
            StandingGate::Decidable => Ok(Authorized {
                _seal: (),
                _not_send: PhantomData,
                principal_id: principal.id().to_string(),
                principal_prov: principal.provenance(),
                over,
            }),
            StandingGate::Judgmental => Err(AuthorizeError::StandingIsJudgmental {
                principal: principal.id().to_string(),
                over: over.to_string(),
            }),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// The authorial gate — standing (one-pool-two-filters .. standing-terminates-at-depth-one, propose-is-authorial)
// ─────────────────────────────────────────────────────────────────────────

/// A principal that may hold **standing** over an object.
///
/// Standing is the authorial condition, and it is the exact mirror of
/// non-identity:
///
/// | gate | condition |
/// |---|---|
/// | judgmental | `π(p) ∩ π(a) = ∅` — **disjointness**. You did not author this. |
/// | authorial | `π(outcome) ⊆ π(p)` and standing — **containment**. It is yours to revise. |
///
/// One pool, two filters; the gate marker selects which predicate applies, not
/// which pool is consulted (one-pool-two-filters). A principal may be both — steward of one
/// container and a disinterested judge of another — and the two facts do not
/// interfere, because they are asked about different objects.
pub trait Steward: Principal {
    /// Whether this principal holds stewardship over the named container.
    ///
    /// The name is the theory's, not Het's: Het requires only that *some*
    /// standing predicate exists (authorial-declares-standing). What counts as standing over what is
    /// the supplying theory's business (nothing-further-required).
    fn has_standing(&self, over: &str) -> bool;
}

/// Why authorization was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum AuthorizeError {
    /// The principal holds no standing over this container.
    NoStanding { principal: String, over: String },
    /// Standing is judgmental in this model and a judge must rule on it first.
    ///
    /// standing-conditional-gated: standing is **conditional-gated** — decidable when provenance
    /// containment settles it, judgmental otherwise. This variant is the
    /// judgmental branch surfacing: the engine cannot settle it and must
    /// dispatch. See [`Pool::classify_standing`].
    StandingIsJudgmental { principal: String, over: String },
}

impl std::fmt::Display for AuthorizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoStanding { principal, over } => {
                write!(f, "{principal} holds no standing over `{over}`")
            }
            Self::StandingIsJudgmental { principal, over } => write!(
                f,
                "standing of {principal} over `{over}` is not settled by provenance \
                 containment; it must be ruled on by a judge (standing-conditional-gated)"
            ),
        }
    }
}

impl std::error::Error for AuthorizeError {}

/// How the standing predicate is settled for a given principal and container.
///
/// standing-conditional-gated makes standing the one **conditional** gate Het actually settles: the
/// mode of satisfaction depends on the specific algebra, and is classified one
/// level up (conditional-names-classifier, conditional-names-classifier).
#[derive(Debug, PartialEq, Eq)]
pub enum StandingGate {
    /// Provenance containment settles it — machine-checked, no outside.
    Decidable,
    /// Containment does not settle it; a judge must rule (terminating at depth
    /// one, standing-terminates-at-depth-one: that judge's own qualification is plain non-identity relative
    /// to the **author**, not to the audited object).
    Judgmental,
}

/// Proof that a principal holds standing to author over a named container.
///
/// The authorial counterpart to [`Qualified`], and sealed for the same reason:
/// a capability that can be fabricated in object-position is not a capability
/// (rung SPEC.md G2). [`Pool::authorize`] is the only mint.
///
/// An `Authorized` is what `propose` and `enact` require — the two authorial
/// operations of the pass (propose-is-authorial). Without one there is no term for "author
/// something about this object."
///
/// Borrowed rather than owned: authorship is not spent by a single act. An
/// author with standing may propose, be rejected, and re-propose (reproposal-carries-the-chain) — the
/// standing did not lapse. This is the deliberate asymmetry with [`Qualified`],
/// which *is* consumed: a judgment licence is spent on one sentence, because
/// each dispatch must re-run the filter against a different argument.
#[must_use = "an Authorized pen is a licence to author; dropping it discards the standing"]
pub struct Authorized<'a> {
    _seal: (),
    _not_send: PhantomData<*const ()>,
    principal_id: String,
    principal_prov: Prov,
    over: &'a str,
}

impl<'a> Authorized<'a> {
    /// The authoring principal's id — for the receipt, and for provenance.
    pub fn principal_id(&self) -> &str {
        &self.principal_id
    }

    /// The author's provenance. A Proposal carries it (proposal-provenance-is-authors).
    pub fn principal_provenance(&self) -> &Prov {
        &self.principal_prov
    }

    /// The container this pen authorizes writing to.
    pub fn over(&self) -> &str {
        self.over
    }
}

impl std::fmt::Debug for Authorized<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Authorized({} over `{}`)", self.principal_id, self.over)
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Proposals — the Proponent's answer (propose-is-authorial, proposal-vocabulary)
// ─────────────────────────────────────────────────────────────────────────

/// **Het declares no edits.**
///
/// edit-required-not-typed: Het requires that a `remedy` carry an edit (remedy-carries-an-edit) and that `enact`
/// apply one (enact-makes-an-endofunctor). It does not enumerate them. `Amend | Remove | Relocate`,
/// `Fix | WontFix | Duplicate`, `Defund` — these are their theories' and are
/// equally Het-shaped.
///
/// An earlier draft of this crate declared an `Edit` enum here. That was a
/// domain vocabulary sitting in the library: it made the cabinet's three cases
/// into Het's three cases, and a triage theory would have had to pretend
/// "won't fix" was an `Amend`.
///
/// The consequence is 11.2 — [`enact`] is **generic over the theory's edit
/// type**, and the library cannot apply an edit it did not name. The domain
/// supplies the application through [`Applies`]. Het governs only who may
/// perform it (one-pool-two-filters) and whether the result is admitted (target-runs-its-own-models).
///
/// This is not a limitation worked around. A formalism that enumerated edits
/// would be legislating domains it does not know.
///
/// What the Proponent says in answer to a non-conforming verdict.
///
/// **proposal-vocabulary** — a Proposal is a *remedy* or a *dispute*. Before the dispute case
/// existed there was no path to contest a verdict: `propose` is defined only on
/// a non-conforming verdict, and the false-positive override lived at `dispose`
/// — downstream. An author who believed the audit simply wrong had to author a
/// remedy for the diagnosis they disputed, in order to obtain a vehicle for
/// disputing it.
///
/// **proposal-provenance-is-authors** — a Proposal carries its **author's** provenance, not the model's.
/// That is what makes `dispose` checkable: the judge must be disjoint from the
/// *proposal* (disjointness-against-argument), and without knowing who authored it the check has nothing
/// to measure against.
///
/// Construction requires an [`Authorized`] pen. `propose` is authorial (propose-is-authorial);
/// there is no term for proposing without standing.
#[derive(Debug, Clone)]
#[must_use = "a Proposal is the Proponent's move; dropping it forfeits the turn"]
pub struct Proposal<E> {
    object: &'static str,
    author: String,
    provenance: Prov,
    kind: ProposalKind<E>,
    /// Which attempt this is. 1 for a first proposal (reproposal-carries-the-chain).
    attempt: usize,
    /// Reasons from prior rejections, oldest first (reason-is-not-an-edit/f).
    prior_reasons: Vec<String>,
}

#[derive(Debug, Clone)]
enum ProposalKind<E> {
    Remedy(E),
    Dispute { grounds: &'static str },
}

impl<E> Provenanced for Proposal<E> {
    /// proposal-provenance-is-authors. A judge disposing on this must be disjoint from *this*, not from
    /// the model — which is why the provenance has to be the author's.
    fn provenance(&self) -> Prov {
        self.provenance.clone()
    }
}

impl<E: Clone> Proposal<E> {
    /// *"The verdict stands; here is the fix."*
    pub fn remedy(pen: &Authorized<'_>, object: &'static str, edit: E) -> Self {
        Self {
            object,
            author: pen.principal_id().to_string(),
            provenance: pen.principal_provenance().clone(),
            kind: ProposalKind::Remedy(edit),
            attempt: 1,
            prior_reasons: Vec::new(),
        }
    }

    /// *"The verdict is wrong; the object stands as authored."*
    ///
    /// Still judged. The author does not overturn a verdict by asserting it —
    /// a dispute goes to `dispose` exactly as a remedy does, and the Opponent
    /// rules on the dispute itself.
    pub fn dispute(pen: &Authorized<'_>, object: &'static str, grounds: &'static str) -> Self {
        Self {
            object,
            author: pen.principal_id().to_string(),
            provenance: pen.principal_provenance().clone(),
            kind: ProposalKind::Dispute { grounds },
            attempt: 1,
            prior_reasons: Vec::new(),
        }
    }

    /// Re-propose after a rejection, carrying the chain (reproposal-carries-the-chain).
    ///
    /// The chain is not bookkeeping. An author re-proposing without the prior
    /// reasons can cycle indefinitely on the same objection, and nothing
    /// downstream could detect it — which is the failure reproposal-carries-the-chain names.
    ///
    /// Takes the pen again because standing must still hold: an author who lost
    /// stewardship between attempts may not continue.
    pub fn reproposed(&self, pen: &Authorized<'_>, ruling: &Ruling<E>, edit: E) -> Self {
        let mut reasons = self.prior_reasons.clone();
        if let Some(r) = ruling.reason() {
            reasons.push(r.to_string());
        }
        Self {
            object: self.object,
            author: pen.principal_id().to_string(),
            provenance: pen.principal_provenance().clone(),
            kind: ProposalKind::Remedy(edit),
            attempt: self.attempt + 1,
            prior_reasons: reasons,
        }
    }

    pub fn object(&self) -> &'static str {
        self.object
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn is_dispute(&self) -> bool {
        matches!(self.kind, ProposalKind::Dispute { .. })
    }

    /// The edit this proposes, if any. A dispute proposes none — there is
    /// nothing to enact.
    pub fn edit(&self) -> Option<&E> {
        match &self.kind {
            ProposalKind::Remedy(e) => Some(e),
            ProposalKind::Dispute { .. } => None,
        }
    }

    pub fn grounds(&self) -> Option<&'static str> {
        match &self.kind {
            ProposalKind::Dispute { grounds } => Some(grounds),
            ProposalKind::Remedy(_) => None,
        }
    }

    /// Which attempt this is (reproposal-carries-the-chain). 1 for a first proposal.
    pub fn attempt(&self) -> usize {
        self.attempt
    }

    /// Reasons from prior rejections, oldest first (reason-is-not-an-edit/f).
    pub fn prior_reasons(&self) -> Vec<&str> {
        self.prior_reasons.iter().map(String::as_str).collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Verdicts
// ─────────────────────────────────────────────────────────────────────────

/// The outcome of `M ⊨ φ` for one sentence.
///
/// Boolean, deliberately and incompletely. Het verdict-space-with-metric requires a verdict space
/// carrying a **metric** `d`, with `ε` reported alongside every verdict as an
/// error bar, so that the satisfaction condition survives renaming (pool-is-parameter). This
/// crate does not implement it, and under a Boolean verdict space that
/// condition does not hold. Named rather than papered over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// `M ⊨ φ`.
    Conforming,
    /// `M ⊭ φ`, with a reason.
    NonConforming { reason: String },
}

impl Verdict {
    pub fn conforming(holds: bool, reason_if_not: impl Into<String>) -> Self {
        if holds {
            Self::Conforming
        } else {
            Self::NonConforming {
                reason: reason_if_not.into(),
            }
        }
    }

    pub fn is_conforming(&self) -> bool {
        matches!(self, Self::Conforming)
    }
}

/// A verdict together with how it was reached.
///
/// The gate is carried on the *result*, not merely on the sentence, so a
/// consumer can tell a machine-check from an outside call without consulting
/// the theory. A judgmental verdict names the principal that produced it; a
/// decidable one has no principal to name, and the type says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    /// Computed inside the algebra. No outside.
    Decidable {
        sentence: &'static str,
        verdict: Verdict,
    },
    /// Obtained from a principal that survived both filters.
    Judgmental {
        sentence: &'static str,
        role: &'static str,
        principal: String,
        verdict: Verdict,
    },
}

impl Settled {
    pub fn verdict(&self) -> &Verdict {
        match self {
            Self::Decidable { verdict, .. } | Self::Judgmental { verdict, .. } => verdict,
        }
    }

    pub fn sentence(&self) -> &'static str {
        match self {
            Self::Decidable { sentence, .. } | Self::Judgmental { sentence, .. } => sentence,
        }
    }

    /// Whether an outside was consulted. The observable form of the gate.
    pub fn consulted_outside(&self) -> bool {
        matches!(self, Self::Judgmental { .. })
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Dispositions — the Opponent's ruling (disposition-is-a-ruling, no-amending-disposition)
// ─────────────────────────────────────────────────────────────────────────

/// The Opponent's ruling on a Proposal.
///
/// **disposition-is-a-ruling: a Disposition is a ruling, not a revision.** The judge classifies;
/// it does not author. That is why the vocabulary is exactly these five and
/// not the six it used to be.
///
/// # What was retired, and why
///
/// `accept-with-mod` is gone. A judge amending a proposal is *transforming*,
/// not classifying — and the judge is provenance-**disjoint** from the object
/// (judgmental-declares-role), so it cannot hold standing over a modification it just authored
/// (authorial-declares-standing). The variant required one principal to satisfy two opposite
/// conditions on one object.
///
/// `reject-with-alternative` fails identically and is not admitted.
///
/// What replaces it: [`Disposition::RejectRemedy`] carrying a **reason** —
/// advisory prose, not an edit (reason-is-not-an-edit). Stating *why* a remedy fails is
/// classification. Supplying the replacement is authorship. The author
/// re-proposes with the reason in hand (reproposal-carries-the-chain).
///
/// # The two rejections
///
/// `reject` used to mean two different things, and the conflation left a
/// dangling object:
///
/// - [`RejectDiagnosis`](Disposition::RejectDiagnosis) — *the audit was
///   wrong.* Terminal, correctly: nothing to enact because nothing is broken.
/// - [`RejectRemedy`](Disposition::RejectRemedy) — *the object is
///   non-conforming; this fix is not acceptable.* **Non-terminal**, or the
///   object is stranded: `propose` is gated on a non-conforming verdict and
///   the loop would have nowhere to go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    /// Execute the Proposal as-is. Terminal, affirming.
    Accept,
    /// The audit was wrong; the object stands as authored. Terminal, not
    /// affirming — nothing is enacted.
    RejectDiagnosis,
    /// The remedy is not acceptable; the object remains non-conforming.
    /// Non-terminal: the author re-proposes, carrying the reason (reason-is-not-an-edit/f).
    RejectRemedy { reason: String },
    /// Cannot act yet; a prerequisite is required. Non-terminal.
    Defer { prerequisite: String },
    /// Needs clarification from the auditor. Non-terminal.
    RaisesQuestions { question: String },
}

impl Disposition {
    /// The vocabulary as `(name, terminal, affirming)`.
    ///
    /// Pinned as data so a change to the vocabulary breaks a test rather than
    /// passing silently. `accept-with-mod` and `reject-with-alternative` are
    /// both absent, for the same reason.
    pub const VARIANTS: &'static [(&'static str, bool, bool)] = &[
        ("accept", true, true),
        ("reject-diagnosis", true, false),
        ("reject-remedy", false, false),
        ("defer", false, false),
        ("raises-questions", false, false),
    ];

    /// **no-bound-on-reentry — Het places no bound on re-entry, deliberately.**
    ///
    /// If no acceptable remedy exists, `reject-remedy` re-enters forever and
    /// the object never leaves the loop. Het does not resolve this and cannot:
    /// every available answer — evict the object, bound the attempts, accept
    /// non-conformance as declared debt — is **worth-shaped**, and cut-at-valuation forbids
    /// a Het theory from declaring a worth-law.
    ///
    /// This is the first case found in which χ alone produces a state it
    /// cannot exit. It is a stated limit, not an oversight. **The bound belongs
    /// in HetOpt**; an implementation that quietly gave up after N attempts
    /// would be smuggling a worth-law in under another name.
    ///
    /// Until HetOpt ships, an implementation must surface a re-entering object
    /// to its outside rather than loop on it.
    pub const REENTRY_BOUND: Option<usize> = None;

    pub fn name(&self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::RejectDiagnosis => "reject-diagnosis",
            Self::RejectRemedy { .. } => "reject-remedy",
            Self::Defer { .. } => "defer",
            Self::RaisesQuestions { .. } => "raises-questions",
        }
    }

    /// Whether the object reaches a fixed point, or re-enters the loop.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Accept | Self::RejectDiagnosis)
    }

    /// Whether this licenses enactment. Only `accept` does.
    ///
    /// Terminality and affirmation are different questions:
    /// `reject-diagnosis` is terminal and enacts nothing.
    pub fn is_affirming(&self) -> bool {
        matches!(self, Self::Accept)
    }

    /// The advisory reason, where one is carried (reason-is-not-an-edit).
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::RejectRemedy { reason } => Some(reason),
            Self::Defer { prerequisite } => Some(prerequisite),
            Self::RaisesQuestions { question } => Some(question),
            _ => None,
        }
    }
}

/// A Disposition together with the proposal it ruled on and the judge that
/// rendered it.
///
/// The judge is recorded because a ruling with no attributable judge cannot be
/// audited for non-identity after the fact — and P0 is exactly the property
/// someone will later want to check.
#[derive(Debug, Clone)]
#[must_use = "a Ruling decides what happens next; dropping it strands the object"]
pub struct Ruling<E> {
    object: &'static str,
    disposition: Disposition,
    judge: String,
    /// The edit the ruling affirms, if it affirms one.
    edit: Option<E>,
}

impl<E> Ruling<E> {
    pub fn object(&self) -> &'static str {
        self.object
    }
    pub fn disposition(&self) -> &Disposition {
        &self.disposition
    }
    pub fn judge(&self) -> &str {
        &self.judge
    }
    pub fn is_terminal(&self) -> bool {
        self.disposition.is_terminal()
    }
    pub fn is_affirming(&self) -> bool {
        self.disposition.is_affirming()
    }
    pub fn reason(&self) -> Option<&str> {
        self.disposition.reason()
    }
    /// The edit this ruling licenses, if any.
    pub fn edit(&self) -> Option<&E> {
        self.edit.as_ref()
    }
}

/// The Opponent rules on a Proposal — **judgmental**.
///
/// Consumes a [`Qualified`] token by value. The token must have been minted
/// against **the proposal** (disjointness-against-argument, [`Pool::qualify_for`]), not against the
/// model: a judge that authored the proposal is disjoint from the model by
/// construction, and a model-relative check would admit it to rule on its own
/// work.
///
/// The disposition comes from the judge. This function records it; nothing
/// here decides.
pub fn dispose<R: Role, E: Clone>(
    proposal: &Proposal<E>,
    _judge: Qualified<R>,
    disposition: Disposition,
) -> Ruling<E> {
    let edit = if disposition.is_affirming() {
        proposal.edit().cloned()
    } else {
        None
    };
    Ruling {
        object: proposal.object(),
        judge: _judge.principal_id().to_string(),
        disposition,
        edit,
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Enactment — the authorial gate, and the write-guard (disposition-is-a-ruling, target-runs-its-own-models)
// ─────────────────────────────────────────────────────────────────────────

/// Why an enactment did not land.
#[derive(Debug, PartialEq, Eq)]
pub enum EnactError {
    /// The Disposition does not license enactment — it is not affirming.
    NotAffirmed { disposition: &'static str },
    /// The pen does not authorize writing to this container.
    NoStandingOverTarget { author: String, target: String },
    /// **target-runs-its-own-models.** The edit was authorized, and the destination's own law
    /// refused it.
    ///
    /// This is the write-guard: where a revised object enters another governed
    /// container, that container's `⊨` runs — the pass composed with itself
    /// under fractal closure (fractal-property). An authorization to edit is *not* a licence
    /// to violate the target's law.
    ///
    /// `enact` therefore has two failure points, not one: the Disposition may
    /// withhold it, and the target may refuse it.
    TargetRefused { target: String, reason: String },
    /// The object named by the ruling is not in the source container.
    ObjectNotFound { object: &'static str },
}

impl std::fmt::Display for EnactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAffirmed { disposition } => {
                write!(f, "`{disposition}` does not license enactment")
            }
            Self::NoStandingOverTarget { author, target } => {
                write!(f, "{author} holds no standing over `{target}`")
            }
            Self::TargetRefused { target, reason } => write!(
                f,
                "`{target}` refused the write on its own law: {reason} (target-runs-its-own-models)"
            ),
            Self::ObjectNotFound { object } => write!(f, "no object `{object}` in the source"),
        }
    }
}

impl std::error::Error for EnactError {}

/// A container governed by its own law, with a boundary that can refuse.
///
/// **Not part of Het's contract with `enact`** — a convenience for domains
/// whose edits move objects between governed containers. `admits` is the
/// target's `⊨` run at the boundary: the write-guard (target-runs-its-own-models), which is the pass
/// composed with itself (gate-law), not new machinery.
///
/// A domain whose edits do not move anything (issue triage closing a ticket)
/// needs none of this.
pub trait Container {
    /// The object type this container holds.
    type Item;

    /// The container's name, as used by [`Steward::has_standing`].
    fn name(&self) -> &'static str;

    /// Remove an object by id, if present.
    fn take(&mut self, id: &str) -> Option<Self::Item>;

    /// **The write-guard.** Would this container's own law admit the object?
    ///
    /// Returns the reason for refusal, or `None` to admit. This is the target's
    /// `⊨` run before the write lands — `pass ∘ pass`, not new machinery.
    fn admits(&self, item: &Self::Item) -> Option<String>;

    /// Accept an object. Called only after `admits` returned `None`.
    fn put(&mut self, item: Self::Item);
}

/// How a domain's edit is applied — **supplied by the theory, not by Het**.
///
/// enact-generic-over-edit. Het requires that `enact` apply an edit (enact-makes-an-endofunctor) and does not enumerate
/// edits (edit-required-not-typed), so the library has nothing to dispatch on: it cannot know
/// that `Relocate` moves and `WontFix` closes. The theory that named the edits
/// says what they do.
///
/// What Het keeps is the part that is Het's: the author must hold standing
/// (one-pool-two-filters), and where the result lands in governed territory that territory's
/// law runs (target-runs-its-own-models). Those are enforced by [`enact`] around this call, not
/// inside it.
///
/// Returning [`EnactError::TargetRefused`] from `apply` is how a domain reports
/// that its own destination declined the write. [`enact`] does not second-guess
/// it.
pub trait Applies<E> {
    /// The territory this world is, as named in a standing predicate (one-pool-two-filters).
    ///
    /// `enact` checks the pen against it. Without this the pen is decorative:
    /// an author with standing over one container could enact on another.
    fn territory(&self) -> &'static str;

    /// Apply `edit` to the object named by `object`.
    ///
    /// Called only after [`enact`] has confirmed the ruling affirms and the pen
    /// authorizes. Provenance and standing are already settled; what remains is
    /// the domain's own law and its own mechanics.
    fn apply(&mut self, object: &'static str, edit: &E) -> Result<(), EnactError>;
}

/// The author applies a ruling — **authorial**.
///
/// Requires an [`Authorized`] pen: `enact` transforms the object, and
/// transformation demands standing over it (one-pool-two-filters), never disjointness.
///
/// Het's two checks, in order:
///
/// 1. the Disposition affirms ([`EnactError::NotAffirmed`]);
/// 2. the pen authorizes this territory
///    ([`EnactError::NoStandingOverTarget`]).
///
/// Then the domain applies its own edit. Whatever the domain's law refuses —
/// including a destination that declines an already-authorized write (target-runs-its-own-models) —
/// comes back as [`EnactError::TargetRefused`].
///
/// **The library performs no edit.** It cannot: it does not know what the
/// theory's edits are. That is 11.2, and it is the whole shape of the split.
pub fn enact<E, W>(
    world: &mut W,
    ruling: &Ruling<E>,
    pen: &Authorized<'_>,
) -> Result<Enacted, EnactError>
where
    W: Applies<E>,
{
    if !ruling.is_affirming() {
        return Err(EnactError::NotAffirmed {
            disposition: ruling.disposition().name(),
        });
    }

    if pen.over() != world.territory() {
        return Err(EnactError::NoStandingOverTarget {
            author: pen.principal_id().to_string(),
            target: world.territory().to_string(),
        });
    }

    let Some(edit) = ruling.edit() else {
        // A dispute affirms nothing to enact (proposal-vocabulary).
        return Ok(Enacted {
            object: ruling.object(),
        });
    };

    world.apply(ruling.object(), edit)?;
    Ok(Enacted {
        object: ruling.object(),
    })
}

/// Evidence that an edit landed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Enacted {
    object: &'static str,
}

impl Enacted {
    pub fn object(&self) -> &'static str {
        self.object
    }
    /// The object that moved — for a relocation.
    pub fn moved(&self) -> &'static str {
        self.object
    }
}

// ─────────────────────────────────────────────────────────────────────────
// The theory! macro
// ─────────────────────────────────────────────────────────────────────────

/// Declare a Het theory — a sort and its gate-marked sentences.
///
/// ```text
/// theory!(Name for ModelType {
///     decidable  sentence_name = |m: &ModelType| -> bool { .. };
///     judgmental sentence_name: RoleType;
/// });
/// ```
///
/// Emits a module (the theory name, lowercased) containing one unit struct per
/// sentence, each with the signature its gate licenses:
///
/// - **decidable** → `fn holds(model: &M) -> Verdict`. No pool parameter, no
///   principal, no capability. The body *cannot* reach an outside because
///   nothing in scope can produce one.
/// - **judgmental** → `fn settle(model: &M, q: Qualified<R>, v: Verdict) -> Settled`,
///   consuming the token by value. Without a [`Qualified`], there is no term.
///
/// This is Het gate-faithful (gate-faithfulness) by construction rather than by audit: an
/// algebra cannot launder a judgmental operation into a decidable one, because
/// the two have different types.
///
/// The `SENTENCES` constant emitted alongside is the theory's `Sen(Σ)` as data —
/// what a pass walks.
#[macro_export]
macro_rules! theory {
    (
        $name:ident for $model:ty {
            $( $rule:tt )*
        }
    ) => {
        #[allow(non_snake_case)]
        pub mod $name {
            use super::*;
            #[allow(unused_imports)]
            use $crate::{Qualified, Settled, Verdict};

            $crate::__sentences!($model ; $( $rule )*);
        }
    };
}

/// Emit one sentence per rule, and `SENTENCES` as the accumulated list.
///
/// Split by gate at the *matcher* rather than by a runtime tag: `decidable`
/// and `judgmental` are different productions of the grammar, so a malformed
/// declaration fails to parse instead of failing later. Rust also forbids an
/// `expr` fragment being followed by `:`, so a single unified rule with both an
/// optional body and an optional role is not expressible — the separation is
/// forced, and is the better shape regardless.
#[doc(hidden)]
#[macro_export]
macro_rules! __sentences {
    // ── accumulate ──────────────────────────────────────────────────────
    ( $model:ty ; $( $rest:tt )* ) => {
        $crate::__sentences_acc!($model ; [] ; $( $rest )*);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __sentences_acc {
    // decidable NAME = |m| ..;
    ( $model:ty ; [ $( $acc:tt )* ] ;
      decidable $sentence:ident = $body:expr ; $( $rest:tt )* ) => {
        $crate::__decidable!($sentence, $model, $body);
        $crate::__sentences_acc!($model ;
            [ $( $acc )* (stringify!($sentence), "decidable"), ] ; $( $rest )*);
    };

    // judgmental NAME: Role;
    ( $model:ty ; [ $( $acc:tt )* ] ;
      judgmental $sentence:ident : $role:ty ; $( $rest:tt )* ) => {
        $crate::__judgmental!($sentence, $model, $role);
        $crate::__sentences_acc!($model ;
            [ $( $acc )* (stringify!($sentence), "judgmental"), ] ; $( $rest )*);
    };

    // ── refusals, with the reason ───────────────────────────────────────

    // judgmental with a body: a judgmental sentence is settled, not computed.
    ( $model:ty ; [ $( $acc:tt )* ] ;
      judgmental $sentence:ident = $body:expr ; $( $rest:tt )* ) => {
        compile_error!(concat!(
            "judgmental sentence `", stringify!($sentence), "` has a body. A judgmental \
             sentence is settled by a principal, not computed — if it can be computed it \
             is decidable, and marking it judgmental launders a machine check into an \
             outside call. Het judgmental-declares-role also requires it declare the competence role needed to \
             discharge it. Write: judgmental ", stringify!($sentence), ": SomeRole;"
        ));
    };

    // decidable with a role: nothing is dispatched, so there is no role.
    ( $model:ty ; [ $( $acc:tt )* ] ;
      decidable $sentence:ident : $role:ty ; $( $rest:tt )* ) => {
        compile_error!(concat!(
            "decidable sentence `", stringify!($sentence), "` declares a role. A decidable \
             sentence is machine-checked and consults no principal, so there is no role to \
             play. Either give it a body (decidable ", stringify!($sentence), " = |m| ..;) \
             or mark it judgmental."
        ));
    };

    // judgmental with neither — the role(phi) gap, refused.
    ( $model:ty ; [ $( $acc:tt )* ] ;
      judgmental $sentence:ident ; $( $rest:tt )* ) => {
        compile_error!(concat!(
            "judgmental sentence `", stringify!($sentence), "` declares no role. Het judgmental-declares-role: \
             every judgmental sentence MUST declare the competence role required to \
             discharge it — it is what lets satisfaction resolve a judge at all. \
             Write: judgmental ", stringify!($sentence), ": SomeRole;"
        ));
    };

    // decidable with neither.
    ( $model:ty ; [ $( $acc:tt )* ] ;
      decidable $sentence:ident ; $( $rest:tt )* ) => {
        compile_error!(concat!(
            "decidable sentence `", stringify!($sentence), "` has no body. A decidable \
             sentence is machine-checked, so it must say what to check. Write: decidable ",
            stringify!($sentence), " = |m| ..;"
        ));
    };

    // ── done ────────────────────────────────────────────────────────────
    ( $model:ty ; [ $( $acc:tt )* ] ; ) => {
        /// `Sen(Σ)` as data — every sentence with its gate. What a pass walks.
        pub const SENTENCES: &[(&str, &str)] = &[ $( $acc )* ];
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __decidable {
    ($sentence:ident, $model:ty, $body:expr) => {
        #[allow(non_camel_case_types)]
        pub struct $sentence;

        impl $sentence {
            pub const NAME: &'static str = stringify!($sentence);
            pub const GATE: &'static str = "decidable";

            /// `M ⊨ φ`, machine-checked.
            ///
            /// Takes only the model. There is no parameter through which a pool,
            /// a principal, or a [`Qualified`] token could enter — which is the
            /// enforcement, not a convention about what the body ought to do.
            pub fn holds(model: &$model) -> $crate::Settled {
                let f: fn(&$model) -> bool = $body;
                $crate::Settled::Decidable {
                    sentence: Self::NAME,
                    verdict: $crate::Verdict::conforming(
                        f(model),
                        concat!(
                            "decidable sentence `",
                            stringify!($sentence),
                            "` does not hold"
                        ),
                    ),
                }
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __judgmental {
    ($sentence:ident, $model:ty, $role:ty) => {
        #[allow(non_camel_case_types)]
        pub struct $sentence;

        impl $sentence {
            pub const NAME: &'static str = stringify!($sentence);
            pub const GATE: &'static str = "judgmental";

            /// `M ⊨ φ`, settled by an outside.
            ///
            /// Consumes the [`Qualified`] token **by value**: a licence is spent
            /// on one sentence and cannot be reused to discharge a second. The
            /// verdict comes from the principal; this records it. Nothing here
            /// fabricates a verdict.
            pub fn settle(
                _model: &$model,
                q: $crate::Qualified<$role>,
                verdict: $crate::Verdict,
            ) -> $crate::Settled {
                $crate::Settled::Judgmental {
                    sentence: Self::NAME,
                    role: <$role as $crate::Role>::NAME,
                    principal: q.principal_id().to_string(),
                    verdict,
                }
            }
        }

        /// `role(φ)` for this sentence — see [`Judgmental`].
        impl $crate::Judgmental for $sentence {
            type Requires = $role;
            const SENTENCE: &'static str = stringify!($sentence);
        }
    };
}
