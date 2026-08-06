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
//! `questions/open/`.

// The principal pool, its two filters, and the capability tokens they mint now
// live in `rung` — the `ladder!` macro's gate markers emit `::rung::Qualified`,
// so the type has to be reachable from the crate that hosts the macro. They are
// re-exported here unchanged: `rung-het` remains their documented home and
// every path below (`rung_het::Pool`, `rung_het::Qualified`, ..) still resolves.
pub use rung::{
    AuthorizeError, Authorized, Consulted, Judgment, Judgmental, OutcomeNotFromJudge, Pool,
    Principal, Prov, Provenanced, Qualified, QualifyError, Raised, Response, Role, SettleError,
    Settled, Situated, StandingGate, Steward, Terminated, TokenNotBound, Verdict,
};

// `theory!` and its helper macros moved to `rung` for the same reason the pool
// did, one axis over: `ladder!` is the DSL's **arrow** surface and `theory!` is
// its **sentence** surface, and Het gate-marks both (gate-marker-required —
// "every sentence and every operation"). A library that wants to declare a
// theory should not have to depend on the crate that hosts the pass. The
// re-export keeps every existing path (`rung_het::theory!`) resolving, and the
// gate law's compile-fail proofs above still exercise this surface through
// `rung_het`.
pub use rung::theory;

// The pass is declared with `ladder!` ([`het_pass!`] below expands to one), so
// the macro has to be reachable from here: a consumer that declares the pass
// should not have to name `rung` as well.
pub use rung::ladder;

// ─────────────────────────────────────────────────────────────────────────
// The chain — what the pass carries back to the authoring position
// (reproposal-carries-the-chain, reason-is-not-an-edit)
// ─────────────────────────────────────────────────────────────────────────

/// The record of how the subject got back to the authoring position.
///
/// **Classification only, and that is a constraint rather than a choice.**
/// `Chain` is the payload of the pass's `Proposing` rung, and `Proposing` is
/// the target of the `RejectRemedy` and `Defer` **continue arms**. A continue
/// arm's target rung is built *inline by `step`* (rung-props.md G10) — that is,
/// by the **judge**. Anything `Chain` could carry, a judge could therefore
/// author. So it carries an id, a container, a count, the audit's diagnosis,
/// and prose: no edit, no proposal, and no type parameter one could hide in
/// (disposition-is-a-ruling, no-amending-disposition, reason-is-not-an-edit).
///
/// What it *must* carry is the chain itself (reproposal-carries-the-chain).
/// Without the prior reasons an author can cycle indefinitely on the same
/// objection and nothing downstream could detect it.
///
/// Note the consequence for re-entry: because the chain **strictly grows**, a
/// `must_progress`-style guard on this edge could never fire.
/// It would be mandatory and vacuous at once — which is the second reason
/// re-entry is a continue arm rather than a recoverable verdict, the first
/// being that such a guard is an eviction rule (guarded-reentry-is-eviction).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chain {
    subject_id: String,
    container: String,
    /// The audit's diagnosis — why the subject is in the loop at all.
    diagnosis: Option<String>,
    /// Which attempt the proposal authored from this chain will be. 1 on entry.
    attempt: usize,
    /// The dispositions already rendered, oldest first — **names only**.
    prior_dispositions: Vec<&'static str>,
    /// The reasons they carried, oldest first. Advisory prose, never an edit.
    prior_reasons: Vec<String>,
}

impl Chain {
    /// The chain a subject enters the authoring position with, first time.
    pub fn opening(subject_id: &str, container: &str, verdict: &Verdict) -> Self {
        Self {
            subject_id: subject_id.to_string(),
            container: container.to_string(),
            diagnosis: match verdict {
                Verdict::Conforming => None,
                Verdict::NonConforming { reason } => Some(reason.clone()),
            },
            attempt: 1,
            prior_dispositions: Vec::new(),
            prior_reasons: Vec::new(),
        }
    }

    /// The chain after a non-terminal ruling — one attempt further on, with the
    /// disposition's name and its reason appended.
    ///
    /// The only thing a [`Disposition`] contributes here is its **name and its
    /// prose**. There is no parameter through which an edit could arrive.
    #[must_use]
    pub fn reentered(&self, disposition: &Disposition) -> Self {
        let mut prior_dispositions = self.prior_dispositions.clone();
        prior_dispositions.push(disposition.name());
        let mut prior_reasons = self.prior_reasons.clone();
        if let Some(r) = disposition.reason() {
            prior_reasons.push(r.to_string());
        }
        Self {
            subject_id: self.subject_id.clone(),
            container: self.container.clone(),
            diagnosis: self.diagnosis.clone(),
            attempt: self.attempt + 1,
            prior_dispositions,
            prior_reasons,
        }
    }

    pub fn subject_id(&self) -> &str {
        &self.subject_id
    }

    /// Which attempt the next proposal is (reproposal-carries-the-chain).
    pub fn attempt(&self) -> usize {
        self.attempt
    }

    /// What the audit said, if it said anything.
    pub fn diagnosis(&self) -> Option<&str> {
        self.diagnosis.as_deref()
    }

    /// The dispositions already rendered, oldest first.
    pub fn prior_dispositions(&self) -> &[&'static str] {
        &self.prior_dispositions
    }

    /// The reasons carried back, oldest first (reason-is-not-an-edit).
    pub fn prior_reasons(&self) -> Vec<&str> {
        self.prior_reasons.iter().map(String::as_str).collect()
    }
}

/// The container the subject sits in — what the authorial pen must be held
/// over (authorial-qualifying-set).
///
/// This is what makes `Proposing`'s payload legal as the source of an
/// `#[authorial(R)]` transition: rung-props.md G14 injects
/// `must_hold_standing_over(&proposing.payload, &pen)` ahead of the body, and
/// without a container there is nothing standing could be held over.
impl Situated for Chain {
    fn container(&self) -> &str {
        &self.container
    }
}

/// What an author answers a verdict with (proposal-vocabulary).
///
/// Exactly two, because a Proposal is exactly two. Separate from [`Proposal`]
/// because the Proposal also carries the author, the provenance and the chain —
/// none of which the author supplies. `Answer` is the part that is theirs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer<E> {
    /// *"The verdict stands; here is the fix."* Carries an **edit**
    /// (remedy-carries-an-edit); the edits are the theory's.
    Remedy(E),
    /// *"The verdict is wrong; the subject stands as authored."* Nothing to
    /// enact, and still judged (dispute-is-still-judged).
    Dispute { grounds: &'static str },
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
    author: String,
    provenance: Prov,
    kind: ProposalKind<E>,
    /// The chain this was authored from (reproposal-carries-the-chain). It also
    /// names the subject and the container, so nothing else has to.
    chain: Chain,
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
    /// The authorial act, in one place: an answer, a pen, and the chain it was
    /// authored from.
    ///
    /// This is what the pass's `#[authorial(R)]` transition calls. The pen is
    /// required because `propose` is authorial (propose-is-authorial) and there
    /// is no term for proposing without standing; the chain is required because
    /// a re-proposal must carry it (reproposal-carries-the-chain).
    pub fn from_chain<R: Role>(pen: &Authorized<'_, R>, chain: &Chain, answer: Answer<E>) -> Self {
        Self {
            author: pen.principal_id().to_string(),
            provenance: pen.principal_provenance().clone(),
            kind: match answer {
                Answer::Remedy(edit) => ProposalKind::Remedy(edit),
                Answer::Dispute { grounds } => ProposalKind::Dispute { grounds },
            },
            chain: chain.clone(),
        }
    }

    /// *"The verdict stands; here is the fix."*
    pub fn remedy<R: Role>(pen: &Authorized<'_, R>, object: &str, edit: E) -> Self {
        Self::from_chain(
            pen,
            &Chain::opening(object, pen.over(), &Verdict::Conforming),
            Answer::Remedy(edit),
        )
    }

    /// *"The verdict is wrong; the object stands as authored."*
    ///
    /// Still judged. The author does not overturn a verdict by asserting it —
    /// a dispute goes to `dispose` exactly as a remedy does, and the Opponent
    /// rules on the dispute itself.
    pub fn dispute<R: Role>(pen: &Authorized<'_, R>, object: &str, grounds: &'static str) -> Self {
        Self::from_chain(
            pen,
            &Chain::opening(object, pen.over(), &Verdict::Conforming),
            Answer::Dispute { grounds },
        )
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
        Self::from_chain(
            pen,
            &self.chain.reentered(ruling.disposition()),
            Answer::Remedy(edit),
        )
    }

    pub fn object(&self) -> &str {
        self.chain.subject_id()
    }

    /// The chain this was authored from (reproposal-carries-the-chain).
    pub fn chain(&self) -> &Chain {
        &self.chain
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
        self.chain.attempt()
    }

    /// Reasons from prior rejections, oldest first (reason-is-not-an-edit/f).
    pub fn prior_reasons(&self) -> Vec<&str> {
        self.chain.prior_reasons()
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Verdicts
// ─────────────────────────────────────────────────────────────────────────

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
    object: String,
    disposition: Disposition,
    judge: String,
    /// The sealed [`Judgment`] of the judge who disposed — its provenance is
    /// the seal's, not a field: Q12's *a judgmental outcome carries its judge's
    /// provenance*, made true of the pass's ruling. A `dispatched` record is
    /// written from this, and nothing can fabricate it.
    judgment: Judgment,
    /// The edit the ruling affirms, if it affirms one.
    edit: Option<E>,
}

impl<E> Ruling<E> {
    pub fn object(&self) -> &str {
        &self.object
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

    /// The sealed judgment of the judge who disposed — provenance out of the
    /// seal, for a `dispatched` record.
    pub fn judgment(&self) -> &Judgment {
        &self.judgment
    }

    /// Take the licence out of an affirming ruling, or `None`.
    ///
    /// The type-level statement of licence-is-not-guarantee: only an affirming
    /// Disposition yields a [`Licence`], and a `Licence` is still only
    /// permission — see [`enact`] for the two ways it fails to land.
    pub fn into_licence(self) -> Option<Licence<E>> {
        Licence::of(self)
    }
}

/// What a terminal, affirming Disposition hands to the author.
///
/// **licence-is-not-guarantee.** A `Licence` is permission to `enact`, not a
/// promise the edit lands. It exists as a type so that the pass's `Accept` arm
/// carries *permission* rather than a revised subject: an
/// `Accept -> Governed` arm would have had the **judge** apply the edit, which
/// disposition-is-a-ruling forbids. `enact` is a separate authorial arrow,
/// consuming this licence and a pen.
///
/// The two ways it still fails are enact-has-two-failure-points, and both live
/// in [`enact`]: the pen may not authorize the territory, and the target may
/// refuse the write on its own law (target-runs-its-own-models).
#[derive(Debug, Clone)]
#[must_use = "a Licence is permission to enact; dropping it forfeits the affirmation"]
pub struct Licence<E> {
    ruling: Ruling<E>,
}

impl<E> Licence<E> {
    /// A licence from an affirming ruling. `None` for every other disposition —
    /// the vocabulary's `affirming` column is the whole condition.
    pub fn of(ruling: Ruling<E>) -> Option<Self> {
        ruling.is_affirming().then_some(Self { ruling })
    }

    /// The ruling this licence rests on.
    pub fn ruling(&self) -> &Ruling<E> {
        &self.ruling
    }

    pub fn into_ruling(self) -> Ruling<E> {
        self.ruling
    }
}

impl<E> std::ops::Deref for Licence<E> {
    type Target = Ruling<E>;
    fn deref(&self) -> &Ruling<E> {
        &self.ruling
    }
}

/// What a terminal, **non**-affirming Disposition hands back.
///
/// The payload of the pass's `RejectDiagnosis` arm: the audit was wrong, so
/// there is nothing to enact and nothing to re-propose. It records who ruled
/// and on what — a ruling with no attributable judge cannot be audited for
/// non-identity after the fact.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "a Why is the terminal record of a rejected diagnosis"]
pub struct Why {
    subject_id: String,
    judge: String,
    reason: String,
}

impl Why {
    /// Read a terminal non-affirming ruling.
    pub fn of<E>(ruling: &Ruling<E>) -> Self {
        Self {
            subject_id: ruling.object().to_string(),
            judge: ruling.judge().to_string(),
            reason: ruling
                .reason()
                .unwrap_or("the audit was wrong; the subject stands as authored")
                .to_string(),
        }
    }

    pub fn subject_id(&self) -> &str {
        &self.subject_id
    }
    pub fn judge(&self) -> &str {
        &self.judge
    }
    pub fn reason(&self) -> &str {
        &self.reason
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
        object: proposal.object().to_string(),
        judge: judge.principal_id().to_string(),
        // The sealed judgment of the adjudicator who disposed — provenance
        // rides out of the seal, so the ruling can substantiate a `dispatched`
        // record and cannot fabricate one.
        judgment: judge.judgment().clone(),
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
    ObjectNotFound { object: String },
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
    fn apply(&mut self, object: &str, edit: &E) -> Result<(), EnactError>;
}

/// The **second face of an edit**: not "apply it" but "is it observably in
/// effect?" — the check an *observer* makes after enact, so success is attested
/// by someone other than the author (`enact-verify`, the third failure point of
/// [`enact`]: the remedy may land as typed yet not be observably present).
///
/// `Applies` mutates; `Verify` reads back. The two are one world when an edit's
/// effect is a mechanical predicate on the object's state — the common,
/// decidable case. A world that cannot confirm an edit (a purely ·runt·
/// effect) simply does not implement this: the driver then **fails closed** and
/// refuses to claim success, rather than taking the author's word.
pub trait Verify<E>: Applies<E> {
    /// Whether `edit` is now observably in effect on `object`, read back from
    /// this world's state — never from the author's report.
    fn confirms(&self, edit: &E, object: &str) -> bool;
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
            object: ruling.object().to_string(),
        });
    };

    world.apply(ruling.object(), edit)?;
    Ok(Enacted {
        object: ruling.object().to_string(),
    })
}

/// Evidence that an edit landed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Enacted {
    object: String,
}

impl Enacted {
    pub fn object(&self) -> &str {
        &self.object
    }
    /// The object that moved — for a relocation.
    pub fn moved(&self) -> &str {
        &self.object
    }
}

// ═════════════════════════════════════════════════════════════════════════
// The pass, as a `ladder!` (the-pass)
// ═════════════════════════════════════════════════════════════════════════

/// Declare the audit–rectify pass for one theory.
///
/// Expands to a single [`ladder!`] declaration. **The spine, the gates and the
/// Disposition vocabulary are Het's**; the three bodies are the theory's.
///
/// ```text
/// carry { subject_id: String, container: String }
/// Governed(Subject)
///   => Audited(Verdict)
///   => Proposing(Chain)                          // classification-only payload
///   => #[authorial(Author)] Proposed(Proposal)   // propose-is-authorial
///   => #[judgmental(Judge)] {
///        Accept(Licence)
///        | RejectDiagnosis(Why)
///        | RejectRemedy    -> Proposing
///        | Defer           -> Proposing
///        | RaisesQuestions -> Audited
///      }
/// ```
///
/// # Two constraints that fall out of the shape
///
/// **[`Proposing`](Chain) carries classification only.** `RejectRemedy` and
/// `Defer` are **continue arms**, and a continue arm's target rung is built
/// inline by `step` — by the *judge* (rung-props.md G10). If that rung's
/// payload held proposal content the judge would be authoring, which
/// disposition-is-a-ruling and no-amending-disposition forbid. So it is a
/// [`Chain`]: a count, an id, a container, and prose.
///
/// **`enact` is not in the branching transition.** `Accept -> Governed` would
/// close the loop inside the ladder — and would have the judge apply the edit,
/// for exactly the same reason. `Accept` is terminal and carries a [`Licence`];
/// [`enact`] is a separate authorial arrow consuming that licence and a pen.
/// The endofunctor of enact-makes-an-endofunctor therefore lives at the level
/// of **composition**, not inside one declaration — see rung-props.md §5.7.
///
/// # Re-entry is unguarded
///
/// `RejectRemedy` / `Defer` are `->`, never `=>`. A recoverable verdict makes
/// the macro inject `must_progress` (rung-props.md G8), which panics on no
/// progress — an eviction rule, which guarded-reentry-is-eviction forbids. And
/// because reproposal-carries-the-chain makes the payload strictly grow, such a
/// guard could never fire: mandatory *and* vacuous.
///
/// # The three bodies
///
/// | body | signature | what it is |
/// |---|---|---|
/// | `audit` | `Fn(&Subject) -> Verdict` | the theory's own law, run on the subject |
/// | `propose` | `Fn(&Chain, &str) -> Answer<Edit>` | the **author's** move; the `&str` is the pen's principal id |
/// | `rule` | `Fn(&Proposal<Edit>, &str) -> Disposition` | the **judge's** ruling; the `&str` is the licensed principal's id |
///
/// The judge is reached by *id* rather than by a static method on the role, so
/// two qualifying judges may rule differently on one argument — the arrow is
/// `A → 𝒫(B)` (judgmental-is-kleisli-arrow), not `A → B`.
///
/// ```
/// use rung_het::*;
///
/// #[derive(Clone, Debug, PartialEq)]
/// pub struct Draft { pub complete: bool }
/// impl Provenanced for Draft { fn provenance(&self) -> Prov { Prov::of(["drafter"]) } }
///
/// #[derive(Clone, Copy)] pub struct Editor;
/// impl Role for Editor { const NAME: &'static str = "editor"; }
/// #[derive(Clone, Copy)] pub struct Reader;
/// impl Role for Reader { const NAME: &'static str = "reader"; }
///
/// #[derive(Clone, Debug, PartialEq, Eq)] pub enum DraftEdit { Finish }
///
/// het_pass!(Pass {
///     subject = Draft,
///     edit = DraftEdit,
///     author = Editor,
///     judge = Reader,
/// } impl {
///     audit = |d: &Draft| Verdict::conforming(d.complete, "unfinished"),
///     propose = |_c: &Chain, _who: &str| Answer::Remedy(DraftEdit::Finish),
///     rule = |_p: &Proposal<DraftEdit>, _who: &str| Disposition::Accept,
/// });
///
/// # fn main() {
/// let entry = pass::Governed::new(
///     Draft { complete: false },
///     pass::Carry { subject_id: "d1".into(), container: "folio".into() },
/// );
/// assert_eq!(pass::proposing(pass::audited(entry)).payload.attempt(), 1);
/// # }
/// ```
#[macro_export]
macro_rules! het_pass {
    (
        $name:ident {
            subject = $subject:ty,
            edit = $edit:ty,
            author = $author:ty,
            judge = $judge:ty $(,)?
        } impl {
            audit = $audit:expr,
            propose = $propose:expr,
            rule = $rule:expr $(,)?
        }
    ) => {
        $crate::ladder!($name {
            carry { subject_id: String, container: String }

            Governed($subject)
              => Audited($crate::Verdict)
              => Proposing($crate::Chain)
              => #[authorial($author)] Proposed($crate::Proposal<$edit>)
              => #[judgmental($judge)] {
                     Accept($crate::Licence<$edit>)
                   | RejectDiagnosis($crate::Why)
                   | RejectRemedy    -> Proposing
                   | Defer           -> Proposing
                   | RaisesQuestions -> Audited
                 }
        } impl {
            // `audit` — unmarked, so it reads as decidable and has no parameter
            // an outside could enter through (rung-props.md G12). A theory whose
            // audit is judgmental settles it through `theory!`'s `settle` and
            // hands the resulting `Verdict` in.
            audited = |governed| {
                let carry = ::core::clone::Clone::clone(governed.carry());
                let verdict = ($audit)(&governed.payload);
                Audited::new(verdict, carry)
            },

            // The subject reaches the authoring position. Nothing is authored
            // here: the chain records why, and how many times.
            proposing = |audited| {
                let carry = ::core::clone::Clone::clone(audited.carry());
                let chain = $crate::Chain::opening(
                    &carry.subject_id,
                    &carry.container,
                    &audited.payload,
                );
                Proposing::new(chain, carry)
            },

            // `propose` — AUTHORIAL (propose-is-authorial). The pen is in the
            // signature, so there is no term for proposing without standing,
            // and G14's injected prologue admits it only over the container the
            // chain names.
            proposed = |proposing, pen| {
                let carry = ::core::clone::Clone::clone(proposing.carry());
                let answer = ($propose)(&proposing.payload, pen.principal_id());
                let proposal = $crate::Proposal::from_chain(&pen, &proposing.payload, answer);
                Proposed::new(proposal, carry)
            },

            // `dispose` — JUDGMENTAL. The licence is in the signature (G12) and
            // G13's injected prologue admits it only against **this Proposal**
            // (disjointness-against-argument): a judge that authored the
            // proposal is disjoint from the *model* by construction, so a
            // model-relative mint would admit it to rule on its own work.
            //
            // Nothing here decides. The disposition comes from the judge; this
            // body routes it, and the routing is the vocabulary
            // (disposition-vocabulary).
            step = |proposed, judge| {
                let carry = ::core::clone::Clone::clone(proposed.carry());
                let proposal = ::core::clone::Clone::clone(&proposed.payload);
                let disposition = ($rule)(&proposal, judge.principal_id());
                let ruling = match $crate::dispose(&proposal, judge, disposition) {
                    ::core::result::Result::Ok(r) => r,
                    ::core::result::Result::Err(e) => {
                        return ::core::result::Result::Err(Failed {
                            token: proposed,
                            error: ::std::string::ToString::to_string(&e),
                        });
                    }
                };
                let ruled = ::core::clone::Clone::clone(ruling.disposition());
                ::core::result::Result::Ok(match ruled {
                    // Terminal, affirming. The licence goes OUT of the ladder;
                    // `enact` is a separate authorial arrow.
                    $crate::Disposition::Accept => StepOutcome::Accept(Accept::new(
                        ruling
                            .into_licence()
                            .expect("`accept` is the affirming disposition"),
                    )),
                    // Terminal, not affirming — nothing to enact.
                    $crate::Disposition::RejectDiagnosis => {
                        StepOutcome::RejectDiagnosis(RejectDiagnosis::new($crate::Why::of(&ruling)))
                    }
                    // Non-terminal: back to the authoring position, unguarded,
                    // carrying the chain (guarded-reentry-is-eviction,
                    // reproposal-carries-the-chain).
                    $crate::Disposition::RejectRemedy { .. } | $crate::Disposition::Defer { .. } => {
                        let chain = proposal.chain().reentered(&ruled);
                        let outcome = Proposing::new(chain, carry);
                        match ruled {
                            $crate::Disposition::Defer { .. } => StepOutcome::Defer(outcome),
                            _ => StepOutcome::RejectRemedy(outcome),
                        }
                    }
                    // Non-terminal, and further back: the auditor clarifies, so
                    // the subject re-enters at `Audited` and the chain restarts.
                    $crate::Disposition::RaisesQuestions { question } => {
                        StepOutcome::RaisesQuestions(Audited::new(
                            $crate::Verdict::NonConforming { reason: question },
                            carry,
                        ))
                    }
                })
            },
        });
    };
}
