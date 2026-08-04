//! rung — a type ladder where the state machine IS the type system.
//!
//! Declare the ladder and its transition logic together. Bodies in the trailing
//! `impl { .. }` block expand *inside* the generated module, so they use the
//! sealed constructors and the macro auto-injects the recovery guard:
//!
//! ```rust,ignore
//! use rung::ladder;
//!
//! ladder!(Work {
//!     carry { task_id: String }
//!     Designed(WorkSpec) => Active(ActiveLoop) => {
//!         Complete | Stalled => Active | BudgetExhausted
//!     }
//!     recover { retry: Stalled => Active }
//! } impl {
//!     active = |designed| { Active::new(/* .. */, designed.carry().clone()) },
//!     step   = |active|   { Ok(StepOutcome::Complete(Complete::new())) },
//!     retry  = |stalled|  { let a = stalled.into_source(); Active::new(/* .. */, a.carry().clone()) },
//! });
//! // start:  let d = work::Designed::new(spec, carry);   // entry ctor is public
//! // drive:  match work::step(work::active(d)) { .. }     // module `pub fn`s
//! ```
//!
//! Omit the `impl { .. }` block for a type-only declaration (structs, verdict
//! enum, and guards, but no transition logic).
//!
//! ## No-silent-drop (`#[must_use]`)
//!
//! Every generated token — rungs, verdicts, `StepOutcome`, and `Failed` — is
//! `#[must_use]`. Rust types are affine (droppable); the linear-token contract is
//! "consumed *exactly* once". Move semantics give "at most once"; `#[must_use]`
//! guards "at least once". Dropping a token is a warning, and an error under
//! `#![deny(unused_must_use)]`.
//!
//! This is load-bearing: the verdict struct below is publicly constructible, so
//! dropping it under `deny(unused_must_use)` must fail to compile. If the
//! `#[must_use]` attribute were ever dropped from the macro's emit, this example
//! would start compiling and the `compile_fail` test would fail.
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! use rung::ladder;
//! struct SpecData;
//! struct LoopData;
//! ladder!(Demo {
//!     Spec(SpecData) => Active(LoopData) => { Converged | Stalled => Active }
//!     recover { stalled: Stalled => Active }
//! });
//! fn abandons_the_outcome() {
//!     demo::Converged::new(); // dropping a #[must_use] verdict — denied
//! }
//! ```
//!
//! ## No external fabrication (SPEC.md G2)
//!
//! With an inline `impl { .. }` block, only the *entry* rung has a public
//! constructor — every downstream rung's `new` is module-private, so no outside
//! code can mint a mid-ladder token. The following must fail to compile:
//!
//! ```compile_fail
//! use rung::ladder;
//! struct SpecData;
//! #[derive(Clone, PartialEq)]
//! struct LoopData;
//! ladder!(Demo {
//!     Spec(SpecData) => Active(LoopData) => { Done | Retry => Active }
//!     recover { retry: Retry => Active }
//! } impl {
//!     active = |s| { Active::new(LoopData) },
//!     step   = |a| { Ok(StepOutcome::Done(Done::new())) },
//!     retry  = |r| { r.into_source() },
//! });
//! fn fabricate() {
//!     // `Active::new` is private to `demo` — cannot fabricate a mid-ladder rung.
//!     let _ = demo::Active::new(LoopData);
//! }
//! ```
//!
//! ## Terminal verdict payloads
//!
//! A terminal verdict may carry a result: `Converged(Report)` generates
//! `Converged { payload: Report }` with `.payload()` / `.into_payload()`, so a run
//! returns a value through the verdict. A *recoverable* verdict may not — it
//! carries its source rung instead — so the following must fail to compile:
//!
//! ```compile_fail
//! use rung::ladder;
//! struct S; struct L; struct Info;
//! ladder!(Bad {
//!     Spec(S) => Active(L) => { Done | Stalled(Info) => Active }
//!     recover { unstall: Stalled => Active }
//! });
//! ```
//!
//! ## Error-path recovery (`Failed(rung) => rung`)
//!
//! `recover { name: Failed(Active) => Active }` recovers from the error path: when
//! a branching transition returns `Err(Failed { token, error })`, this edge takes
//! the unconsumed `token` back and produces the next rung. No progress guard is
//! injected (a retry after a transient error may legitimately reuse the token).
//! The named rung must exist — this must fail to compile:
//!
//! ```compile_fail
//! use rung::ladder;
//! struct S; struct L;
//! ladder!(Bad {
//!     Start(S) => Working(L) => { Done }
//!     recover { clear: Failed(Nonexistent) => Working }
//! });
//! ```
//!
//! ## Continue arms (`Name -> Rung`)
//!
//! A branching arm written with `->` (produces) instead of `=>` (recover) is a
//! *continue* arm: `step` builds the next rung itself and the `StepOutcome` variant
//! carries it directly — no recover fn, no progress guard, no source-carrying.
//! `Tick -> Counting` gives `StepOutcome::Tick(Counting)`; the driver just
//! reassigns. The target rung must exist — this must fail to compile:
//!
//! ```compile_fail
//! use rung::ladder;
//! struct S;
//! ladder!(Bad {
//!     Begin(S) => Counting(i32) => { Tick -> Nonexistent | Done }
//! });
//! ```

pub use rung_macro::ladder;

// ════════════════════════════════════════════════════════════════════════════
// The principal pool
// ════════════════════════════════════════════════════════════════════════════
//
// Het's outside — the principal pool, the two admissibility filters, and the
// sealed capability tokens they mint. These live here rather than in `rung-het`
// because the `ladder!` macro's gate markers emit them: a `#[judgmental(R)]`
// transition takes a `::rung::Qualified<R>`, so the type must be reachable from
// the crate the macro's users already depend on. `rung-het` re-exports every
// item below, so it remains their documented home.

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

// Compile-check and run the README's code blocks as doctests, so the README
// cannot silently drift from the macro. `#[cfg(doctest)]` means this item exists
// only during doctest builds — it never appears in the public API or on docs.rs.
// Illustrative README blocks are fenced ```rust,ignore; the Getting Started
// example is a complete ```rust program that is compiled and run.
#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
struct ReadmeDoctests;
