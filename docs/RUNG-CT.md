# RUNG-CT — how the category was found

> **Status: development archaeology. Not normative.**
>
> This is the working document in which rung's categorical account was
> derived — the discovery that a `ladder` declaration *is* a free category,
> and the successive resolutions (Q7, Q9, Q10) that established what sits
> above and around it. It records *how the account was arrived at*,
> including alternatives considered and rejected, the reviews that settled
> each question, and the claims that had to be withdrawn.
>
> **The normative statement is
> [`rung-ct-propositions.md`](rung-ct-propositions.md).** Where the two
> disagree, `rung-ct-propositions.md` governs. A claim stated here and not
> there is not a claim rung makes.
>
> Two things here remain load-bearing and are cited as such:
>
> - **The resolution records (§4–§6)** — `questions/resolved/q7`, `q9`, `q10`
>   and their `_evidence/` name this document as the fold target. Those
>   citations point *here* deliberately: they record the reasoning, not the
>   requirement.
> - **The corrections register (§8)** — what the earlier revisions of this
>   document claimed, and why each claim was withdrawn. A superseded claim
>   that leaves no trace comes back.

---

## 1. The origin — the law was found from the inside

rung is a **category-declaration language**. A ladder declares the objects and
the legal arrows of a category, and the type system enforces that you may travel
only declared arrows. This is not an analogy the way "it's *like* a state
machine" is an analogy — it is what the primitive **is**.

That was not read from theory and implemented. It was found from the inside.

An attempt to fold a live LLM verdict into a ladder tried to **construct the
next state to hold the verdict**, and the sealed constructor refused:
`Active::new` cannot be called from outside the arrow (E0624, pinned by
`rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624`
and illustrated by the `rung/src/lib.rs` compile_fail doctest). The refusal looked at first like an ergonomic
annoyance — a fabrication guard being over-strict.

It was not. A morphism was being asked for in object-position, and no such
thing exists in a category. The fix was forced and correct: the verb moved into
the transition body, the only place a verb can be.

That episode is the whole reason the categorical account is taken seriously
here. The theory did not predict the refusal; the refusal was hit first, and the
theory was what explained it. It is normative as
[the law](rung-ct-propositions.md#the-law), and it is what
`SPEC.md` G2 and `conformance.md` cite by name.

The same refusal appears on a second axis in Het — an algebra runs its own
decidable step but cannot construct the state that holds a judgmental outcome
([`self-governing-not-self-closing`](rung-het-propositions.md#self-governing-not-self-closing)).
Het's gate law and rung's law are one move made twice: seal the capability, hand
it only to the arrow licensed to hold it.

---

## 2. The running example

Every example in this document is the `Opt` ladder from
`rung/tests/end_to_end.rs`. It is a real, compiled, driven test, not a sketch —
which matters, because earlier revisions of this document ran two mutually
incompatible examples and both were ungrammatical.

```rust
ladder!(Opt {
    carry { budget: u32 }

    Spec(SpecData)
      => Active(LoopState)
      => {
          Iterating => Active   // recoverable: loop back for another step
          | Converged(Report)   // terminal success, carrying a result payload
          | Exhausted           // terminal failure
      }

    recover {
        iterate: Iterating => Active
    }
} impl { /* active, step, iterate */ });
```

Reading it as a category presentation:

| declaration | category |
|---|---|
| `Spec`, `Active` | objects (rungs) |
| `Spec => Active` | a generating morphism, named `active` — after its **target**, lowercased |
| `Active => { .. }` | the branching morphism, always named `step` |
| `Iterating`, `Converged`, `Exhausted` | summands of the branching coproduct |
| `Failed<Active>` | the residual summand, the fourth thing `step` can return |
| `iterate: Iterating => Active` | the backward edge — the dagger, guarded |
| `carry { budget }` | the product factor carried by every object |

The naming rule is `SPEC.md` §1: a forward transition is named after its
**target** rung lowercased; the branching transition is always `step`. Earlier
revisions wrote `design: Designed → Claimed`, which no ladder ever emits.

A continue arm is the same ladder with one character changed:

```
Iterating -> Active     // continue arm: `step` builds the next Active inline
Iterating => Active     // recoverable verdict: hands off to the guarded recover fn
```

`->` reads *produces*; `=>` reads *recover*. The difference is the whole of §5
below.

---

## 3. Where each proposition came from

A short derivation index. The claim itself lives in
`rung-ct-propositions.md`; this is where it came from.

| propositions | origin |
|---|---|
| the free category, the law | the sealed-constructor refusal, §1 |
| the coproduct | the observation that a verdict variant addition breaks every `match` — i.e. that exhaustiveness is a universal property, not a lint |
| the product / carry | `carry` was designed as a correlation-key channel; the product reading came afterwards and immediately exposed [3.41](rung-ct-propositions.md#constancy-is-not-enforced) as a limit, not a law |
| the indexed monad | the type of a transition function, read with its endpoints as indices |
| the writer / trace | the Python reference interpreter (`.archive/python-poc/rung/interpreter.py`) accumulates one; the macro does not |
| the Prism | **Q7**, §4 |
| the dagger | conjectured from the `recover` block's shape; vindicated and *situated* by Q7 |
| affine substrate | Rust's borrow checker, read as linear logic minus weakening's absence |
| Curry–Howard | standard |
| the opfibration | **Q9**, §5 |
| the composite opfibration | **Q10**, §6 |

---

## 4. Q7 — effectful bodies: which monad?

*Resolved 2026-07-18. Record: `questions/resolved/q7-effectful-bodies-which-monad.md`
and its `_evidence/`.*

**The question.** A fallible transition returns `Result<StepOutcome, Failed<A>>`
— it hands the *input token back* on failure. A handoff conjecture proposed that
this made the transition a Kleisli arrow for an effect monad, and that async
recovery was therefore already "half-written in the recover edges": one monad,
one story, error and async unified.

**The falsifier proposed with the conjecture.** Does `Failed<A>`-returns-input
fit a Kleisli composition, or a dagger?

**The answer: neither the conjecture nor the plain dagger.** Two independent
expert reviews converged:

- `_evidence/q7-kleisli-review-1-dagger.md` — *"Does **not** hold. The structure
  is the **dagger** (partial, resource-preserving recovery on the failure
  injection), not a Kleisli arrow for any standard effect monad. The 'return the
  input' design is exactly what enables the dagger and is load-bearing for
  optional recovery."*
- `_evidence/q7-kleisli-review-2-prism.md` — *"The §6 Dagger is vindicated. Your
  RUNG-CT intuition was exactly correct. Recovery is the backward pass of a
  Prism. The 'well-founded progress guard' (G8) simply ensures the optic's
  backward pass is contractive rather than a symmetric involution."*

So the dagger was not replaced — it was **situated**. A transition is a Prism (a
dependent optic, Capucci–Hedges); the `recover` edges are its Build pass; the
failure-returns-input design is the optic's *residual*.

**The clean proof that it is not a monad**, which is the falsifier triggering:
compose `f: A → B + A` with `g: B → C + B`. A failing `g` hands back `B`, but the
composite's domain is `A`. No monadic `bind` can route `B → A` — only an explicit
backward edge can.

**Two consequences, both now normative.**

- G8 is the contraction, not a defect
  ([`verdict-dagger-is-contractive`](rung-ct-propositions.md#verdict-dagger-is-contractive)).
  A Prism's Build pass need not be involutive; the progress guard makes it
  *contractive*, which is why `f†† = f` is deliberately broken.
- Effects layer on the forward pass
  ([`effects-layer-on-the-forward-pass`](rung-ct-propositions.md#effects-layer-on-the-forward-pass)).
  Async is `T = Future`, a strong commutative monad — tensorial strength
  `A ⊗ T(B) → T(A ⊗ B)` is exactly what proves linearity survives `.await`. A
  generative body is a Markov kernel, an affine probability monad in Fritz's
  sense.

**Alternative rejected.** The Kleisli "unification" — error and async as one
monad — is **false**. Error is the optic's backward pass; effects are monads on
its forward pass; they are orthogonal gadgets needing no distributive law. Async
is therefore a free-standing feature, tracked as **Q8**, and unblocked by nothing
in the error structure.

---

## 5. Q9 — the dependency superstructure

*Resolved 2026-07-18. Record: `questions/resolved/q9-the-dependency-superstructure.md`
and its `_evidence/`.*

**The question.** A ladder is one category. But ladders — and questions, claims,
decisions — depend on each other. The growth tower (`questions/_map.md`)
predicted a Level-1 superstructure would live there; what *is* it?

**The answer: a Grothendieck opfibration.** Two independent reviews named it
precisely and converged, differing only on fibration-vs-opfibration, which
resolved toward the sharper review.

- `_evidence/q9-review-1-fibration.md` — *"RUNG-CT can now name Level ≥1: the
  dependency superstructure **is** the fibration of states (and obligations) over
  the free category of typed edges."*
- `_evidence/q9-review-2-opfibration.md` — *"The dependency network is a
  Grothendieck Opfibration over a category of Dependent Optics. The fractal
  architecture is not just an analogy; it is mathematically rigorous."*

**Orientation is the load-bearing detail.** Transport runs *forward*, with the
direction of information flow — a pushforward, opcartesian. A plain fibration
would pull back, and would be the wrong structure.

**The sharp falsifier, and why it does not fire.** A `premise` edge propagates
deterministically, but a `justification` edge relies on a human judging "no
change needed." A rigid state-assignment functor `B → Set` cannot map a source's
change into a state the target is *already* in without violating the free-category
laws — so advisory propagation *appears* to break strict functoriality.

It does not, and the reason is the coproduct. An advisory pushforward does not
land in the target's rungs; it lands in a **coproduct** — `ReviewRequired +
Survives` — which the base delivers rigidly. Collapsing that coproduct is the
target's own Level-0 work. And because every fibre is a free category, the target
retains *vertical* morphisms of its own; lifts evaluate against its current
state. Normative as
[`advisory-lift-lands-in-a-coproduct`](rung-ct-propositions.md#advisory-lift-lands-in-a-coproduct)
and
[`same-coproduct-at-both-levels`](rung-ct-propositions.md#same-coproduct-at-both-levels).

**Edge types as pushforwards.** The operative registry consequence is
`EDGES.md`; the taxonomy is the registry's, not the formalism's
([`edge-type-selects-the-pushforward`](rung-ct-propositions.md#edge-type-selects-the-pushforward)).

| edge type | what the pushforward does |
|---|---|
| `premise` | strict lift → an obligatory re-examination |
| `justification` | coproduct lift → review-required + survives (advisory) |
| `spawn` | the dependent exists only as the source's child |
| `citation` | a mechanical state update, no human in the loop |

**Blast radius is the backward pass.** Before modifying an item, you query
backward along the composite optic; the answer is a typed *exposure vector* —
*"3 mechanical updates (cheap), 2 obligatory coproduct reviews (expensive)."*
`questions/_reach.py` computes the **deflationary Boolean shadow** of that today:
it walks reachability and prints a checklist. The store (frontmatter now, a graph
store eventually) is inconsequential; the model is what is named.

---

## 6. Q10 — the tower iterates

*Resolved 2026-07-19. Record: `questions/resolved/q10-fractal-registry-hierarchy.md`
and its `_evidence/`.*

**The question.** §5 describes the opfibration over *one* base — the item graph
of a single registry. Does the structure iterate up a domain hierarchy
(`relational-being ⊐ {memory, actions, …}`, each domain carrying its own
registry)?

**The answer: yes, exactly, with no new machinery.** Two independent CT reviews
converged. Let `q : B → B′` map items to their parent domains and `p : E → B` be
the opfibration of §5. **Opfibrations are closed under composition** (Bénabou;
Jacobs, *Categorical Logic and Type Theory*, Lemma 1.1.4), so

```
E  ─p→  B  ─q→  B′          (state ⊐ item ⊐ sub-domain ⊐ domain)
```

is a single composite Grothendieck opfibration. "An opfibration whose fibres are
opfibrations" collapses into one unified opcartesian structure.

Three consequences, all confirmed by review and all now normative:

- **It is an iteration of Level 1, not a Level 2**
  ([`iteration-not-a-second-level`](rung-ct-propositions.md#iteration-not-a-second-level)).
  Opfibrations are 1-cells in **Cat** and compose by ordinary functor
  composition. The reserved Level-2 slot in `_map.md` stays **vacant** — filling
  it needs a genuine 2-cell *between* fibrations (a schema migration, a topology
  remap), which nesting does not introduce.
- **Obligation-transport is scale-invariant**
  ([`transport-is-scale-invariant`](rung-ct-propositions.md#transport-is-scale-invariant)).
  Optics compose (Capucci–Hedges; Spivak's *Poly*), so `_reach.py` and its
  typed-exposure successor cross domain boundaries with **zero modification**.
- **Horizontal ≅ vertical**
  ([`horizontal-and-vertical-coincide`](rung-ct-propositions.md#horizontal-and-vertical-coincide)).
  Under the Grothendieck construction the indexed hierarchy flattens into a
  single total graph; sibling edges and domain-implication edges lift
  identically.

**The one divergence, and how it resolved.** Review 2 said "build the hierarchy
now"; Review 1 said "description, not mandate." That resolves by distinguishing
**theorem** from **application**: the categorical claim is a theorem (composition
of structures §5 already names) and resolves on the reviews. *Building* a domain
registry hierarchy is gated by the third-instance rule in `_map.md`, on lived
need, not on the proof.

---

## 7. Three decisions taken in the split

The normative document was not a transcription. Three positions were decided
while writing it, and each changed a claim.

### 7.1 G2 is a limit, not a blanket refusal

The earlier text said the compiler "refuses any attempt to construct a morphism
not in the free category." That is **false as stated**. In
`rung-macro/src/lib.rs`:

```rust
let ctor_vis = if has_bodies && !is_entry { quote! {} } else { quote! { pub } };
```

and the same rule for verdicts. Without an inline `impl` block, `has_bodies` is
false and **every constructor is `pub`** — a type-only declaration publishes the
whole category to external construction, and no diagnostic fires.

`SPEC.md` G2's phrasing was already careful about this ("*When an `impl` block is
present*, only the entry rung's `new` is public"); this document's was not. The
resolution is a pair: freeness is claimed
([`category-is-freely-generated`](rung-ct-propositions.md#category-is-freely-generated)),
and the reach of its enforcement is stated as a limit
([`freeness-enforced-only-with-bodies`](rung-ct-propositions.md#freeness-enforced-only-with-bodies)),
with the entry constructor and the module boundary as the two further
qualifications.

### 7.2 Continue arms are objects in the coproduct

Continue arms (`V -> R`, `SPEC.md` G10) were absent from every earlier revision
of this document. They are not a variant of the dagger. The account:

A continue arm's summand carries an **object of the category**, not a verdict
object. So a continue arm is an ordinary generating morphism whose *selection* is
deferred to the coproduct — the morphism into the target rung is taken on the
forward pass; which summand was taken is what the eliminator learns.

It needs no backward edge because **it never leaves the category**: nothing to
return from, and no round trip for a contraction to shrink. `SPEC.md` §2 rule 4
exempting continue arms from recover-pairing is not a special case; it is this.

Two consequences follow, and both were missing:

- the branching coproduct is **heterogeneous** — verdict summands, object
  summands, and the residual are three different kinds of thing
  ([`coproduct-is-heterogeneous`](rung-ct-propositions.md#coproduct-is-heterogeneous));
- there are **three shapes of loop-back and only two are daggers**
  ([`three-shapes-of-loop-back`](rung-ct-propositions.md#three-shapes-of-loop-back)).

### 7.3 "Provenance" was two concepts sharing a word

This document used *provenance* for the execution trace.
`rung-het-propositions.md` uses it for **authorship tags**, whose disjointness is
what decides non-identity — Het's P0, the thing the whole formalism exists to
enforce. Unrelated concepts, one word, across two documents in the same slug
space.

rung's own type is already called `Trace`. The execution sense is renamed to
**trace** throughout both halves of RUNG-CT, and the distinction is stated
normatively rather than left to the reader
([`trace-is-not-authorship-provenance`](rung-ct-propositions.md#trace-is-not-authorship-provenance)).

Het's usage is unchanged, and the `Provenanced` trait in `rung-het` is
**deliberately not renamed** — that is the authorship sense and it is correct.

---

## 8. Corrections register

Claims the earlier revisions of this document made, which do not survive into
the normative half. Each was checked against `rung-macro/src/lib.rs`, `SPEC.md`,
or the test suite.

| withdrawn claim | what is true | evidence |
|---|---|---|
| "the compiler refuses **any** out-of-category construction" | only with an inline `impl` block; a type-only declaration publishes every constructor | `rung-macro/src/lib.rs` — `ctor_vis` |
| two example ladders, `Designed/Claimed/Active` and `Spec/Active` | one running example (§2); the earlier two were mutually incompatible **and both ungrammatical** — the verdict block is mandatory | `SPEC.md` §1 grammar |
| a forward transition is named `design: Designed → Claimed` | named after its **target**, lowercased — so `claimed`, or in the running example `active`. The branching transition is always `step` | `SPEC.md` §1; `rung-macro/src/lib.rs` |
| the carry is "structurally shared (duplicated by reference)" | copied **by value** into each object's own `carry` field | `rung-macro/src/lib.rs` — `carry: Carry` field + ctor init |
| the carry satisfies comonad coassociativity | not enforced — a body supplies the successor's carry and may change it, and the running example's `iterate` decrements `budget` | `rung/tests/end_to_end.rs` |
| `f† ∘ f ≠ id` (on tokens) | the guard compares **payloads**, never tokens; a token comparison would be vacuous | `rung-macro/src/lib.rs` — `must_progress(&__before, &__after.payload)` |
| G4 covers "every rung and verdict type" | also `StepOutcome` and `Failed` | `SPEC.md` G4; `rung-macro/src/lib.rs` |
| the coproduct diagram's injection arrows | injections point **into** the coproduct; elimination is the unique morphism out. The ASCII diagram had them reversed, and the diagram is dropped rather than redrawn | — |
| the verification table omitted G3, G10, G11 | all three are in [10.1](rung-ct-propositions.md#guarantees-carry-categorical-content); G3 has a real categorical reading — one token cannot be driven by two threads | `SPEC.md` G3/G10/G11 |
| §10 "resolves on the reviews" as a claim inside the theory | an epistemic-status claim about a document, not a claim about the category. It belongs here (§6), not there | — |

---

## 9. Alternatives considered and rejected

- **Error as a Kleisli arrow.** The Q7 conjecture. Rejected by the composition
  argument in §4: no `bind` routes `B → A`.
- **One monad for error and async.** Rejected with it. They are orthogonal
  gadgets; async needs no distributive law with the error structure, which is
  what makes Q8 a free-standing feature rather than a blocked one.
- **A fibration (rather than an opfibration) for dependencies.** Review 1's
  framing. Rejected in favour of Review 2's: transport runs *with* information
  flow, so the lifts are opcartesian.
- **A Level-2 structure for the domain hierarchy.** Rejected by Q10: nesting
  opfibrations composes 1-cells and yields a 1-cell. A second level needs a
  2-cell between fibrations, which nesting does not supply.
- **A clean (involutive) dagger.** Rejected deliberately. `f†† = f` would permit
  a stall loop that type-checks; the progress guard trades symmetry for
  well-foundedness.
- **Modelling continue arms as an unguarded dagger.** Rejected in the split
  (§7.2): a dagger is a return, and a continue arm never departs.
- **Putting the categorical vocabulary on the surface.** Rejected as the whole
  point — the mathematics is the implementation, not the surface. A surface that
  required it would make the enforcement rest on the author restating it
  correctly.

---

## 10. Where the account is thin

Recorded so it is not mistaken for completeness.

- **The trace is structural only.** The macro emits nothing; the writer
  correspondence describes a trace wherever a caller accumulates one. The Python
  reference interpreter does.
- **True no-drop linearity** is a language-level want, tracked as **Q3**
  (`questions/blocked/q3-true-no-drop-linearity.md`). `#[must_use]` is the affine
  approximation and is escapable.
- **Async** is **Q8**, open, unblocked by Q7.
- **Gate-faithfulness** — Het's requirement that an algebra declare which arrows
  are judgmental — has no counterpart in the ladder DSL. `conformance.md` records
  it as the largest unclosed distance between Het and rung.
- **The domain hierarchy is proved, not built.** Q10 is a theorem; no registry
  hierarchy exists, and building one is gated by the third-instance rule.

---

*"The category theory is the compiler's internal representation. The ladder
syntax is the surface. The programmer thinks in rungs; the compiler thinks in
indexed monads. That's the gap."* — Forge ⚒️, 2026-07-16
