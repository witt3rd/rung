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
//! The explicit `fn main` is load-bearing: without it rustdoc wraps the whole
//! snippet in one, the `ladder!`-generated `mod demo` no longer sees the
//! function-local `SpecData`/`LoopData`, and the example fails on `E0425`
//! instead of on `unused_must_use` — a green test asserting nothing. The
//! diagnostic itself is pinned by
//! `spec_refusals.rs::dropping_a_verdict_under_deny_must_use_is_an_error`,
//! because rustdoc does not check the error code (§6 of rung-props.md).
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
//! fn main() {}
//! ```
//!
//! ## No external fabrication (rung-props.md G2)
//!
//! With an inline `impl { .. }` block, only the *entry* rung has a public
//! constructor — every downstream rung's `new` is module-private, so no outside
//! code can mint a mid-ladder token. The following must fail to compile — with
//! `E0624`, and with nothing else. The `fn main` keeps the items at module
//! scope; without it rustdoc wraps them in a function body and three `E0425`
//! resolution errors join the one the example exists to show, so the example
//! would keep failing even if the constructor were made public. The diagnostic
//! is pinned by `spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624`.
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
//! fn main() {}
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
// transition takes a `::rung::Qualified<R>` and an `#[authorial(R)]` one takes
// a `::rung::Authorized<'_, R>`, so the types must be reachable from the crate
// the macro's users already depend on. `rung-het` re-exports every item below,
// so it remains their documented home.

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
    /// The *authorial* condition on the **outcome** (Het
    /// admissibility-subcategories): `π(f(a)) ⊆ π(p)`. The asymmetry is the
    /// point — judgment demands disjointness, authorship demands containment —
    /// and omitting the second makes the first look like the only option.
    ///
    /// Not read by [`Pool::authorize`], which filters on *capability and
    /// standing* (authorial-qualifying-set) — the condition on the **input**.
    /// The containment condition constrains what an authorial arrow returns,
    /// which is a body property; see rung-props.md §5.
    pub fn contained_in(&self, other: &Prov) -> bool {
        self.0.is_subset(&other.0)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A thing that carries provenance. Subjects and principals both do.
pub trait Provenanced {
    fn provenance(&self) -> Prov;
}

/// A thing that sits in a named container — what standing is held **over**.
///
/// The authorial counterpart to [`Provenanced`], and the reason there are two
/// traits rather than one. The two filters read two different coordinates of
/// the same subject (one-pool-two-filters):
///
/// | filter | coordinate | condition |
/// |---|---|---|
/// | judgmental | `π(a)` — who wrote it | `π(p) ∩ π(a) = ∅`, **disjointness** |
/// | authorial | the container it sits in | `capable(p, role(o)) ∧ standing(p, ·)` |
///
/// [`Pool::authorize`] mints a pen *over a named container*
/// (authorial-qualifying-set); this trait is how a subject says which container
/// that must be. Without it the pen is decorative — an author with standing
/// over one container could revise a subject sitting in another, and nothing
/// could notice.
///
/// **Why not reuse "contained".** Het already uses containment for
/// `π(outcome) ⊆ π(p)` (admissibility-subcategories) — a relation between
/// *provenance sets*, on the way **out** of an arrow. Container membership is a
/// different relation, on the way **in**. Naming them both "containment" would
/// merge two conditions the formalism keeps apart, so this one is *situated*:
/// the subject is somewhere, and standing is held over that somewhere.
pub trait Situated {
    /// The container this subject sits in, as named in a standing predicate.
    ///
    /// The name is the supplying theory's, not Het's, exactly as
    /// [`Steward::has_standing`]'s is (nothing-further-required).
    fn container(&self) -> &str;
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

/// Proof that a principal qualified to judge **this argument** under role `R`.
///
/// This is the crate's load-bearing type. It has no public constructor: the
/// only way to obtain one is [`Pool::qualify_for`], which runs the competence
/// filter and the non-identity filter and refuses on either. Sealed exactly as
/// rung seals its rungs (rung-props.md G2), and for the same categorical reason — a
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
///
/// # The token witnesses a **pair**
///
/// Het non-identity-by-construction: the token witnesses the principal *and*
/// **the argument it was measured against**, and the operation that consumes it
/// admits it only for that argument. A token recording only the principal is
/// unforgeable but **unbound** — it proves someone passed the filter, not that
/// they passed it against *this* argument, so it can be earned against one
/// argument and spent on another. That is the act
/// disjointness-against-argument forbids. Sealing the constructor closes
/// fabrication; [`argument_prov`](Qualified::argument_provenance) closes
/// transfer.
///
/// The binding is a value, not a lifetime brand. A brand would index the token
/// by a scope rather than by the argument, force every consumer into a
/// scoped-closure API, and change every signature `ladder!` emits. What is
/// actually required is `π(p) ∩ π(a) = ∅` **for this `a`** — a statement about
/// provenance, which is what is recorded and what is compared.
#[must_use = "a Qualified token is a licence to judge; dropping it discards the outside"]
pub struct Qualified<R: Role> {
    _seal: (),
    _not_send: PhantomData<*const ()>,
    principal_id: String,
    principal_prov: Prov,
    /// `π(a)` — the provenance of the argument disjointness was measured
    /// against. The half of the pair that closes transfer.
    argument_prov: Prov,
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

    /// `π(a)` — the provenance of the argument this licence was measured
    /// against (non-identity-by-construction).
    pub fn argument_provenance(&self) -> &Prov {
        &self.argument_prov
    }

    pub fn role_name(&self) -> &'static str {
        R::NAME
    }

    /// Whether this licence was minted against **this** argument.
    ///
    /// The predicate half of [`admit`](Qualified::admit), for callers that want
    /// to branch rather than to refuse.
    #[must_use]
    pub fn is_bound_to(&self, argument: &dyn Provenanced) -> bool {
        self.argument_prov == argument.provenance()
    }

    /// Admit this licence **for this argument**, or refuse it.
    ///
    /// The consuming half of non-identity-by-construction. Every operation that
    /// spends a `Qualified` calls this first: the filter ran against some
    /// argument, and the only argument the token licences is that one. A
    /// mismatch is [`TokenNotBound`] — a value the caller cannot drop silently,
    /// not a boolean it may ignore.
    ///
    /// Returns the token on success so it can still be spent; the check is a
    /// gate on the way in, not a copy.
    pub fn admit(self, argument: &dyn Provenanced) -> Result<Self, TokenNotBound> {
        let applied_to = argument.provenance();
        if self.argument_prov == applied_to {
            Ok(self)
        } else {
            Err(TokenNotBound {
                principal: self.principal_id,
                role: R::NAME,
                minted_against: self.argument_prov,
                applied_to,
            })
        }
    }
}

impl<R: Role> std::fmt::Debug for Qualified<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Qualified<{}>({})", R::NAME, self.principal_id)
    }
}

/// A licence was spent on an argument it was not measured against.
///
/// **P0, at the point of use.** The qualification filter ran and the principal
/// passed it — against something else. disjointness-against-argument requires
/// `π(p) ∩ π(a) = ∅` for the very `a` the operation is applied to, so a licence
/// earned elsewhere discharges nothing here.
///
/// This is the refusal that closes *transfer*. The seal on [`Qualified`] closes
/// *fabrication*; the two are different failures and need different mechanisms.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "a token-binding refusal is a P0 violation; ignoring it re-opens the transfer hole"]
pub struct TokenNotBound {
    /// The principal that holds the licence.
    pub principal: String,
    /// The role the licence names.
    pub role: &'static str,
    /// `π(a)` at the mint.
    pub minted_against: Prov,
    /// `π(a)` at the point of use.
    pub applied_to: Prov,
}

impl std::fmt::Display for TokenNotBound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "P0: {}'s `{}` licence was minted against provenance {:?} and is being \
             spent on an argument with provenance {:?}; disjointness is measured \
             against the argument the operation is applied to \
             (rung-het-props.md#disjointness-against-argument)",
            self.principal, self.role, self.minted_against, self.applied_to
        )
    }
}

impl std::error::Error for TokenNotBound {}

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

    /// Het dispatch-is-two-operations — the qualifying set for **this
    /// argument**, then *any* member of it.
    ///
    /// ```text
    /// qualifying = { p ∈ 𝒫 : capable(p, role(φ)) ∧ π(p) ∩ π(a) = ∅ }
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
    ///
    /// This is the mint, and the only one. The token it returns records `π(a)`
    /// alongside the principal (non-identity-by-construction), so which
    /// argument was measured is a fact the token carries rather than a fact the
    /// caller is trusted to have got right — see [`Qualified::admit`].
    ///
    /// **disjointness-against-argument / argument-governs.** Disjointness is
    /// measured against the argument the operation is applied to, never against
    /// "the model" in general. At `audit` the two coincide (`π(a) = π(M)`); at
    /// `dispose` they do not — the argument is a Proposal, whose provenance is
    /// its author's (proposal-provenance-is-authors). A judge that authored a
    /// Proposal is disjoint from the *model* by construction, so a
    /// model-relative check admits it to rule on its own work.
    pub fn qualify_for<R: Role>(
        &self,
        argument: &dyn Provenanced,
    ) -> Result<Qualified<R>, QualifyError> {
        let arg_prov = argument.provenance();

        // Refuse before filtering. With empty argument provenance every
        // candidate passes disjointness and the filter becomes ornamental.
        if arg_prov.is_empty() {
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
            if p_prov.overlaps(&arg_prov) {
                let shared: Vec<String> = p_prov.0.intersection(&arg_prov.0).cloned().collect();
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
                argument_prov: arg_prov,
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

    /// The `audit` reading of [`qualify_for`](Pool::qualify_for): the argument
    /// **is** the model.
    ///
    /// argument-governs: where the argument is the subject under audit,
    /// `π(a) = π(M)` and the two readings coincide. There is one filter, and
    /// this name records which case the caller believes it is in.
    ///
    /// It used to be the other way round — `qualify_for` was a pure alias for
    /// `qualify`, so "against the argument" lived entirely in which reference
    /// the caller happened to pass, and nothing downstream could tell a
    /// model-relative mint from an argument-relative one. It can now: the token
    /// records `π(a)`, and [`Qualified::admit`] refuses it anywhere else. The
    /// caller's choice of name is a comment; the recorded `π(a)` is the check.
    pub fn qualify<R: Role>(&self, model: &dyn Provenanced) -> Result<Qualified<R>, QualifyError> {
        self.qualify_for::<R>(model)
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

    /// Mint an [`Authorized`] pen — the authorial filter, **both conjuncts**.
    ///
    /// ```text
    /// P_auth(o, M) = { p ∈ 𝒫 : capable(p, role(o)) ∧ standing(p, M) }
    /// ```
    ///
    /// The mirror of [`Pool::qualify_for`], and the mirror is not a rename.
    /// Both filters run over the *same* pool and both open with the *same*
    /// competence test; they differ in the second conjunct, and the two second
    /// conjuncts are opposites (one-pool-two-filters):
    ///
    /// | filter | second conjunct |
    /// |---|---|
    /// | judgmental | `π(p) ∩ π(a) = ∅` — you did **not** author this |
    /// | authorial | `standing(p, M)` — this is **yours to revise** |
    ///
    /// Judgment refuses the audited party; authorship requires standing over it
    /// (judgment-refuses-authorship-requires). A principal that qualifies as a
    /// judge of a subject has, on that evidence, said nothing whatever about
    /// its standing over it — and typically the reverse, since the author of a
    /// candidate *is* the party under audit
    /// (provenance-overlap-is-the-point).
    ///
    /// **The competence conjunct is not optional.** `role(o)` is what an
    /// authorial operation needs *done*, exactly as `role(φ)` is for a
    /// judgmental one; dropping it would mint a pen for anyone who holds
    /// standing, however unable to exercise it, and the declared role would be
    /// enforced in name only. This is the conjunct
    /// `gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one`
    /// exists to defend.
    ///
    /// Refuses on the judgmental branch rather than guessing. When containment
    /// does not settle standing, Het says a judge must rule on it
    /// (standing-conditional-gated) — and this engine cannot invent that
    /// ruling. Surfacing [`AuthorizeError::StandingIsJudgmental`] is the honest
    /// outcome; closing it requires the outside, terminating at depth one
    /// (standing-terminates-at-depth-one), and that judge's own qualification
    /// is plain non-identity relative to the **author**
    /// (standing-judge-disjoint-from-author).
    pub fn authorize<'a, R: Role, S: Steward>(
        &self,
        principal: &'a S,
        over: &'a str,
    ) -> Result<Authorized<'a, R>, AuthorizeError> {
        // Competence first — the conjunct both filters share, and the one that
        // reads only the declared interface.
        if !principal.capable(R::NAME) {
            return Err(AuthorizeError::NotCapable {
                principal: principal.id().to_string(),
                role: R::NAME,
            });
        }

        match self.classify_standing(principal, over) {
            StandingGate::Decidable => Ok(Authorized {
                _seal: (),
                _not_send: PhantomData,
                principal_id: principal.id().to_string(),
                principal_prov: principal.provenance(),
                over,
                _role: PhantomData,
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
    /// `capable(p, role(o)) = false` — the competence filter, which the
    /// authorial gate shares with the judgmental one
    /// (authorial-qualifying-set).
    ///
    /// Standing without competence is not authorship. A principal may hold
    /// stewardship over a container and still be unable to do the thing the
    /// object needs done; the qualifying set is a **conjunction**, and this is
    /// the left conjunct failing.
    NotCapable {
        principal: String,
        role: &'static str,
    },
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
            Self::NotCapable { principal, role } => write!(
                f,
                "{principal} is not capable of role `{role}`; standing without \
                 competence is not authorship (authorial-qualifying-set)"
            ),
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

/// Proof that a principal is capable of role `R` **and** holds standing to
/// author over a named container.
///
/// The authorial counterpart to [`Qualified`], and sealed for the same reason:
/// a capability that can be fabricated in object-position is not a capability
/// (rung rung-props.md G2). [`Pool::authorize`] is the only mint.
///
/// An `Authorized` is what `propose` and `enact` require — the two authorial
/// operations of the pass (propose-is-authorial) — and what an
/// `#[authorial(R)]` `ladder!` transition takes as its second parameter
/// (rung-props.md G14). Without one there is no term for "author something about this
/// object."
///
/// # The pen witnesses a **pair**, and it is not the judge's pair
///
/// [`Qualified`] records the principal and `π(a)`, the provenance disjointness
/// was measured against. This records the principal and `over` — the container
/// standing was measured against. Same shape, opposite content, because the two
/// filters read opposite conditions over one pool
/// (one-pool-two-filters, judgment-refuses-authorship-requires). A pen that
/// recorded only the principal would be unforgeable but **unbound**: it would
/// prove someone holds standing somewhere, not that they hold it *here*, and it
/// could be earned over one container and spent on another. [`over`](Self::over)
/// and [`authorizes`](Self::authorizes) close that, exactly as
/// `argument_provenance` and `admit` close it on the judgmental side.
///
/// # Parameterized by the role
///
/// `R` is `role(o)`, the competence the object needs (authorial-qualifying-set).
/// It is a type parameter rather than a field for the same reason it is one on
/// [`Qualified`]: a pen minted for one competence is not the pen another
/// operation asks for, and rustc — which has never heard of Het — is what says
/// so.
///
/// # Borrowed, not owned
///
/// Authorship is not spent by a single act. An author with standing may
/// propose, be rejected, and re-propose (reproposal-carries-the-chain) — the
/// standing did not lapse. This is the deliberate asymmetry with [`Qualified`],
/// which *is* consumed: a judgment licence is spent on one sentence, because
/// each dispatch must re-run the filter against a different argument. The
/// library's own authorial operations (`Proposal::remedy`, `enact`) therefore
/// take `&Authorized`. A `ladder!` transition takes one by value, because a
/// transition consumes its inputs — but nothing was spent, and the same
/// principal may mint another from the pool on the next rung.
#[must_use = "an Authorized pen is a licence to author; dropping it discards the standing"]
pub struct Authorized<'a, R: Role> {
    _seal: (),
    _not_send: PhantomData<*const ()>,
    principal_id: String,
    principal_prov: Prov,
    over: &'a str,
    _role: PhantomData<R>,
}

impl<R: Role> Authorized<'_, R> {
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

    /// `role(o)` — the competence this pen witnesses.
    pub fn role_name(&self) -> &'static str {
        R::NAME
    }

    /// Whether this pen authorizes writing to **this** subject's container.
    ///
    /// The authorial mirror of [`Qualified::is_bound_to`]. Standing was settled
    /// against one container; the only subjects this pen licenses are the ones
    /// sitting in it. A pen spent elsewhere is a write nobody was authorized to
    /// make, whatever the ruling said.
    #[must_use]
    pub fn authorizes(&self, subject: &dyn Situated) -> bool {
        self.over == subject.container()
    }
}

impl<R: Role> std::fmt::Debug for Authorized<'_, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Authorized<{}>({} over `{}`)",
            R::NAME,
            self.principal_id,
            self.over
        )
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Sentences — the `theory!` surface
// ════════════════════════════════════════════════════════════════════════════
//
// `ladder!` is the **arrow** surface of the DSL; `theory!` is the **sentence**
// surface. Het gate-marks both (gate-marker-required: "every sentence and every
// operation"), so the two belong in one crate: a consumer that declares a
// ladder and a consumer that declares a theory depend on the same thing.
//
// `rung-het` re-exports every item below and remains their documented home —
// the gate law, its compile-fail proofs, and the pass (`propose`/`dispose`/
// `enact`) all live there. What moved here is only what a *declaration* needs.

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
///
/// This is the **sentence** surface of the DSL, sibling to [`ladder!`]'s
/// **arrow** surface. Both live in `rung` because Het gate-marks both
/// (gate-marker-required), and because a library that declares a theory should
/// not have to depend on the crate that hosts the pass.
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
            /// a principal, or a `Qualified` token could enter — which is the
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
            /// Consumes the `Qualified` token **by value**: a licence is spent
            /// on one sentence and cannot be reused to discharge a second. The
            /// verdict comes from the principal; this records it. Nothing here
            /// fabricates a verdict.
            ///
            /// The token is admitted only for **this** model
            /// (non-identity-by-construction). A licence minted against another
            /// model is `TokenNotBound` — the argument governs, so an unbound
            /// token discharges nothing here. This is why a judgmental
            /// sentence's sort must be `Provenanced`: without `π(a)` there is
            /// nothing to admit the token against.
            pub fn settle(
                model: &$model,
                q: $crate::Qualified<$role>,
                verdict: $crate::Verdict,
            ) -> ::core::result::Result<$crate::Settled, $crate::TokenNotBound> {
                let q = $crate::Qualified::admit(q, model)?;
                ::core::result::Result::Ok($crate::Settled::Judgmental {
                    sentence: Self::NAME,
                    role: <$role as $crate::Role>::NAME,
                    principal: q.principal_id().to_string(),
                    verdict,
                })
            }
        }

        /// `role(φ)` for this sentence — see [`Judgmental`].
        impl $crate::Judgmental for $sentence {
            type Requires = $role;
            const SENTENCE: &'static str = stringify!($sentence);
        }
    };
}

// Compile-check and run the README's code blocks as doctests, so the README
// cannot silently drift from the macro. `#[cfg(doctest)]` means this item exists
// only during doctest builds — it never appears in the public API or on docs.rs.
// Illustrative README blocks are fenced ```rust,ignore; the Getting Started
// example is a complete ```rust program that is compiled and run.
#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
struct ReadmeDoctests;
