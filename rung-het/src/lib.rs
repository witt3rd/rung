//! rung-het — Het's gate-marked satisfaction, enforced by the type system.
//!
//! ## What this is
//!
//! Het (see `witt3rd/heteronomy`, `docs/formalism.md`) extends institution
//! theory at exactly one point: the satisfaction relation `M ⊨ φ`. Every
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
//! `RUNG-CT.md` §1 states the law rung already enforces: *a verb can only live
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
//! it needs (Het N3) and can only be discharged by a principal that filter
//! [`Pool::qualify`] admitted.
//!
//! ## Scope
//!
//! ## The gate law, as compile errors
//!
//! rung's own discipline (SPEC.md §6): a guarantee that no test can break is
//! not a guarantee. Each case below is a `compile_fail` doctest — if the
//! enforcement is ever weakened, the example starts compiling and the test
//! fails.
//!
//! ### A decidable body cannot reach an outside
//!
//! Het N24 (gate-faithfulness): a `decidable` operation must factor through
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
//! Het N7/N8 — **P0**. `settle` consumes a [`Qualified`], and [`Qualified`] has
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
//! Het N3. This is the gap that went unnoticed in a prose encoding — the map
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
//! a machine check into an outside call, which is the gate law (N27) read at
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
/// Het N20: the base category carries a provenance map `π_X: X → Prov` into a
/// discrete category of tags. Both gate filters read it. Concretely a finite
/// set of identifiers, and disjointness is decidable over finite sets (N7).
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
    /// The *authorial* condition (Het N22). Unused here: the authorial gate is
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
// Roles — Het N3 / N6c
// ─────────────────────────────────────────────────────────────────────────

/// A **competence role** — what a judgmental sentence needs done.
///
/// Het N3: every judgmental operation MUST declare the competence role required
/// to discharge it. N6c pins `capable` to exactly one arity — `𝒫 × Role` — so
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
/// needed to discharge it (N3). Implemented automatically by `theory!` for
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
// Principals — Het §2.2, the interface
// ─────────────────────────────────────────────────────────────────────────

/// An inhabitant of the pool `𝒫` — a concrete outside.
///
/// Het N6b: the theory requires exactly four predicates of a principal
/// supplier and **nothing further**. No kinds, no substrates, no identity
/// fields. `capable` and `π` are here; `standing` belongs to the authorial gate
/// (out of scope) and `ε` to the verdict metric (out of scope).
pub trait Principal: Provenanced {
    /// Het's `capable(p, Role)`, at its one arity (N6c).
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
/// That is Het N7/N8 — P0 — as a property of the type rather than of a code
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

    /// Het N39 — the qualifying set, then *any* member of it.
    ///
    /// ```text
    /// qualifying = { p ∈ 𝒫 : capable(p, role(φ)) ∧ π(p) ∩ π(M) = ∅ }
    /// return any(qualifying)
    /// ```
    ///
    /// **Any**, not the best. Het N9 forbids tiering, costing, or preferring
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
}

// ─────────────────────────────────────────────────────────────────────────
// Verdicts
// ─────────────────────────────────────────────────────────────────────────

/// The outcome of `M ⊨ φ` for one sentence.
///
/// Boolean, deliberately and incompletely. Het N15 requires a verdict space
/// carrying a **metric** `d`, with `ε` reported alongside every verdict as an
/// error bar, so that the satisfaction condition survives renaming (§3). This
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
/// This is Het N24 (gate-faithfulness) by construction rather than by audit: an
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
             outside call. Het N3 also requires it declare the competence role needed to \
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
            "judgmental sentence `", stringify!($sentence), "` declares no role. Het N3: \
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
