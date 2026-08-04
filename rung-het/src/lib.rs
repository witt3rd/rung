//! rung-het — Het's gate-marked satisfaction, enforced by the type system.
//!
//! ## What this is
//!
//! Het (see `witt3rd/heteronomy`, here as `docs/rung-het-props.md`)
//! extends institution theory at exactly one point: the satisfaction relation `M ⊨ φ`. Every
//! sentence carries a **gate marker** fixing *how* satisfaction is computed:
//!
//! | gate | how `M ⊨ φ` is settled |
//! |---|---|
//! | `decidable` | machine-checked. A pure function of the model. |
//! | `judgmental` | dispatched to a **principal** the model did **not** author. Its verdict *is* the outcome. |
//! | `authorial` | dispatched to a principal that holds **standing** over the subject. It transforms rather than classifies. |
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
//! `rung-ct-props.md` states the law rung already enforces (`the-law`):
//! *a verb can only live
//! on a morphism, never inside an object* — enforced by sealed constructors
//! (rung-props.md G2). Het's gate law is the same move on a second axis: **an outside
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
//! rung's own discipline (rung-props.md fractal-property): a guarantee that no test can break is
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
//! The seal (rung rung-props.md G2, applied to the capability rather than the rung).
//! If this compiled, P0 would be a convention.
//!
//! The literal must name **every** field of `Qualified`, including
//! `argument_prov`. An incomplete literal fails with `E0063` (missing field)
//! whether or not the fields are private — so it would keep failing with the
//! seal removed, and would assert nothing. Complete, the sole error is `E0451`,
//! which is exactly the seal. Pinned by
//! `gate_markers.rs::a_qualified_token_cannot_be_constructed_outside_the_pool`.
//!
//! ```compile_fail
//! use rung_het::{Qualified, Role};
//! #[derive(Clone, Copy)]
//! pub struct R;
//! impl Role for R { const NAME: &'static str = "r"; }
//! fn forge_a_licence() -> Qualified<R> {
//!     Qualified { _seal: (), _not_send: std::marker::PhantomData,
//!                 principal_id: "me".into(), principal_prov: Default::default(),
//!                 argument_prov: Default::default(),
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
//! Deliberately partial. Implemented: the `decidable`, `judgmental` and
//! `authorial` gates, both filters over the one pool, and `role(φ)` / `role(o)`.
//! The authorial gate reaches `ladder!` as `#[authorial(Role)]` (rung rung-props.md
//! G14), which emits an `Authorized<'_, Role>` pen and a standing prologue the
//! body cannot skip. Not implemented: the `conditional` gate (classified one
//! level up, and the first place Het has not decided what the encoding needs),
//! the verdict metric `d`, and `ε`. The gap between Het's gate-faithfulness
//! requirement and what a marker can deliver is tracked as Q11, under
//! `docs/questions/open/`.

// The principal pool, its two filters, and the capability tokens they mint now
// live in `rung` — the `ladder!` macro's gate markers emit `::rung::Qualified`,
// so the type has to be reachable from the crate that hosts the macro. They are
// re-exported here unchanged: `rung-het` remains their documented home and
// every path below (`rung_het::Pool`, `rung_het::Qualified`, ..) still resolves.
pub use rung::{
    AuthorizeError, Authorized, Pool, Principal, Prov, Provenanced, Qualified, QualifyError, Role,
    Situated, StandingGate, Steward, TokenNotBound,
};

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
    pub fn remedy<R: Role>(pen: &Authorized<'_, R>, object: &'static str, edit: E) -> Self {
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
    pub fn dispute<R: Role>(
        pen: &Authorized<'_, R>,
        object: &'static str,
        grounds: &'static str,
    ) -> Self {
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
    pub fn reproposed<R: Role>(
        &self,
        pen: &Authorized<'_, R>,
        ruling: &Ruling<E>,
        edit: E,
    ) -> Self {
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
/// Consumes a [`Qualified`] token by value, and **admits it only for this
/// proposal**. The token must have been minted against the proposal
/// (disjointness-against-argument, [`Pool::qualify_for`]), not against the
/// model: a judge that authored the proposal is disjoint from the model by
/// construction, and a model-relative check would admit it to rule on its own
/// work.
///
/// That last sentence used to be advice. It is now the first statement of the
/// body: [`Qualified::admit`] compares the token's recorded `π(a)` against this
/// proposal's, and a licence minted elsewhere comes back as [`TokenNotBound`]
/// (non-identity-by-construction). Sealing the constructor closed fabrication;
/// this closes transfer.
///
/// The disposition comes from the judge. This function records it; nothing
/// here decides.
pub fn dispose<R: Role, E: Clone>(
    proposal: &Proposal<E>,
    judge: Qualified<R>,
    disposition: Disposition,
) -> Result<Ruling<E>, TokenNotBound> {
    // P0 at the point of use. Before this line, `dispose` trusted the caller to
    // have minted against the right argument.
    let judge = judge.admit(proposal)?;

    let edit = if disposition.is_affirming() {
        proposal.edit().cloned()
    } else {
        None
    };
    Ok(Ruling {
        object: proposal.object(),
        judge: judge.principal_id().to_string(),
        disposition,
        edit,
    })
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
pub fn enact<E, W, R: Role>(
    world: &mut W,
    ruling: &Ruling<E>,
    pen: &Authorized<'_, R>,
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
            ///
            /// The token is admitted only for **this** model
            /// (non-identity-by-construction). A licence minted against another
            /// model is [`TokenNotBound`](rung_het::TokenNotBound) — the
            /// argument governs, so an unbound token discharges nothing here.
            /// This is why a judgmental sentence's sort must be
            /// [`Provenanced`](rung_het::Provenanced): without `π(a)` there is
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
