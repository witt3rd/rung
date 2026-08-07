---
id: q11
status: open
depends_on:
  - {on: q1, kind: justification}
affects:
  - {target: q12, kind: premise}
---

# Q11 — Gate-faithfulness *(open)*

**Status:** OPEN

**Standing as of 2026-08-04.** Blocker (1) — the returned value — is **closed**
by [Q12](q12-admissibility-value-or-dispatch.md)'s ruling (R2), and
closed *by derivation* rather than by a guard on the condition itself. Blocker
(2) — `#[conditional(..)]` — **stands**, so this question stays open and
`gate-faithful` stays `parked`. The detail is folded in below rather than
summarised here.

**Question.** Het requires every algebra in `Mod(Σ)` to be **gate-faithful**
([`gate-faithful`](../../../docs/rung-het-props.md#gate-faithful)): every decidable operation factors
through `η`, every judgmental one is a judgmentally-admissible Kleisli arrow,
every authorial one an authorially-admissible one. A ladder declaration carries
**no gate marker**, so an algebra cannot say which of its transitions are
judgmental, and nothing checks that any of them are what they claim.

**Can a gate marker on a transition deliver gate-faithfulness — or only half of
it?**

## Why it's open

The question is not "should `ladder!` take a marker." That much is settled: a
marker emits a different transition signature, and mis-marking then fails to
typecheck rather than being a promise someone keeps
(`rung-het-props.md#mismarking-is-not-a-false-claim`). The open part is
that **gate-faithfulness as Het states it is a property of the interpretation,
not of the declaration.**

Admissibility is a condition on provenance at the point of use —
`π(f(a)) ∩ π(a) = ∅` — which is about the values an arrow actually returns, not
about its type. So a marker makes the *signature* honest. It does not make the
*arrow* admissible. Two halves — and, once a second gate exists, the same two
halves again for it:

| half | what would close it | status |
|---|---|---|
| the signature is honest | a gate marker; a decidable position admits no principal parameter | **built** — rung-props.md G12 |
| the arrow is admissible | the qualifying token is constructible only by the filter, and is **bound** to the argument it was measured against | **built** — rung-props.md G13 |
| both halves, for the *authorial* gate | a pen minted only by the standing filter, admitted only over the container the subject sits in | **built** — rung-props.md G14 |

All three are now built. The conjunction is **not** gate-faithfulness. The
argument is below, and G14 did not change it — it widened the row it did not
close.

## Does half one + half two = gate-faithfulness? No.

This section discharges the standing complaint that nobody had argued it either
way. The answer is no, and the reason is that the table above mislabels its
second row: what G13 delivers is not "the arrow is admissible."

**What was built.** For the judgmental gate: `Qualified<R>` records the principal *and* `π(a)`, the
provenance of the argument disjointness was measured against. `Qualified::admit`
is the only gate that spends it, and every consumer runs it: `dispose` against
the Proposal, `theory!`'s `settle` against the model, and — for a
`ladder!` transition — a macro-injected prologue the domain's body cannot skip,
on the G8 `must_progress` precedent. So a licence earned honestly against one
argument is refused everywhere else.

For the authorial gate, the same shape with the opposite predicate:
`Authorized<'a, R>` records the principal *and* the container standing was
measured over, `Pool::authorize` is the only mint and runs
`capable(p, role(o)) ∧ standing(p, M)`, and an `#[authorial(R)]` transition
carries a macro-injected prologue that admits the pen only over the container
its subject sits in. So a pen earned honestly over one container is refused
everywhere else.

**What that is.** It is P0, completely:
[`non-identity-before-dispatch`](../../../docs/rung-het-props.md#non-identity-before-dispatch)
and [`non-identity-by-construction`](../../../docs/rung-het-props.md#non-identity-by-construction)
now hold as type-and-runtime facts rather than as caller discipline. No
judgmental arrow can be traversed except by a principal drawn from
$\mathcal{P}_{\text{judg}}(\varphi, a)$ for the very $a$ it is applied to.

**Why that is not gate-faithfulness.**

1. ~~**Admissibility constrains the arrow's *output*, and this constrains its
   *input*.**~~ **CLOSED** by [Q12](q12-admissibility-value-or-dispatch.md)'s
   ruling — **R2**, adopt verdict provenance — and it is worth being precise
   about *how*, because it did not close the way this question expected.

   It did **not** close by an epilogue guard on the stated condition. The new
   proposition
   [`judgment-provenance-is-the-judges`](../../../docs/rung-het-props.md#judgment-provenance-is-the-judges)
   obliges a judgmental arrow's outcome to carry its judge's provenance,
   $\pi(f(a)) \subseteq \pi(p)$ — the judgmental mirror of
   [`proposal-provenance-is-authors`](../../../docs/rung-het-props.md#proposal-provenance-is-authors)
   — and *that* is what is asserted, by `theory!`'s `settle` and by the
   epilogue `ladder!` injects on a forward judgmental transition
   ([G15](../../../docs/rung-props.md#g15-outcome-provenance)). Since G13 already
   enforces $\pi(p) \cap \pi(a) = \emptyset$ for the very argument the arrow
   is applied to,

   $$\pi(f(a)) \subseteq \pi(p) \ \wedge\ \pi(p) \cap \pi(a) = \emptyset \implies \pi(f(a)) \cap \pi(a) = \emptyset$$

   so [`admissibility-subcategories`](../../../docs/rung-het-props.md#admissibility-subcategories)'s
   judgmental clause is a **theorem of two enforced facts**. Nothing calls
   `Prov::overlaps` on the way out, deliberately: asserting the conclusion of a
   derivation whose premises are both enforced reads as a third guarantee and
   is none.

   What made the guard sound is what this section's *Received advisory input*
   identified and could not supply — a payload whose provenance is not freely
   chosen by the body. `rung::Judgment` is that payload: sealed, minted only by
   `Principal::judgment`, which calls the new oracle `Principal::rule`. A body
   may still decide *what* comes back; it cannot decide *whose* provenance the
   return carries.

   **What is left of this blocker, narrowed.** Two outward conditions are still
   the body's, and they are recorded at
   [`outward-conditions-remaining`](../../../docs/rung-props.md#outward-conditions-remaining)
   rather than absorbed into the closure: the **authorial** conjunct
   $\pi(f(a)) \subseteq \pi(p)$, which G14 left to the body exactly as G13 left
   the judgmental one, and **branching** judgmental transitions, whose
   recoverable and continue arms carry the argument onward by design
   ([`reproposal-carries-the-chain`](../../../docs/rung-het-props.md#reproposal-carries-the-chain))
   so that an epilogue there would refuse re-entry rather than laundering. The
   first is a hole and is parked on
   `gate_markers.rs::an_authorial_arrow_may_not_return_a_provenance_its_author_does_not_hold`;
   the second is a question and is not settled here. Neither is what this
   blocker said, which was that *nothing* constrained the returned value.

2. **One of the four gates has no encoding at all** — down from two, and this
   blocker **stands**. It is now the only one, and it is what keeps
   `gate-faithful` and `mod-only-gate-faithful` parked.
   `#[authorial(Role)]` is implemented (rung-props.md G14): it emits an
   `Authorized<'_, Role>` pen, `Pool::authorize` runs **both** conjuncts of
   [`authorial-qualifying-set`](../../../docs/rung-het-props.md#authorial-qualifying-set),
   and a macro-injected prologue admits the pen only over the container the
   subject sits in. So an algebra with an authorial operation can now state the
   property — for that operation's *input*. `#[conditional(..)]` remains a
   parse-time refusal, and gate-faithfulness quantifies over *every* operation,
   so an algebra with a conditional operation still cannot state it here at all.

   Two things this did **not** do, and they are why the blocker is half-closed
   rather than closed. It did not touch blocker (1): the authorial half of
   [`admissibility-subcategories`](../../../docs/rung-het-props.md#admissibility-subcategories)
   is `π(f(a)) ⊆ π(p) ∧ standing(p, a)`, and G14 secures the standing conjunct
   on the way *in* while leaving the containment conjunct on the way *out*
   entirely to the body — the same shape as G13's gap, on the second gate.
   `Prov::contained_in` exists and no guarantee calls it. And it did not close
   the *conditional* branch of
   [`standing-conditional-gated`](../../../docs/rung-het-props.md#standing-conditional-gated):
   where provenance containment does not settle standing,
   `Pool::authorize` returns `AuthorizeError::StandingIsJudgmental` rather than
   minting a pen. That refusal is honest and is tested, but Het says a judge
   rules there
   ([`standing-terminates-at-depth-one`](../../../docs/rung-het-props.md#standing-terminates-at-depth-one)),
   and rung has no term for that dispatch.

3. ~~**`decidable` still does not factor through $\eta$.**~~ **CLOSED** — see
   *Received advisory input* below. The argument was an over-read on our part:
   $\eta$ is the unit of $\mathcal{P}$, so "factors through $\eta$" *is*
   $\mathcal{P}$-purity and never claimed absolute world-purity. A decidable
   transition may still read a clock, and that is
   [`purity-not-secured`](../../../docs/rung-het-props.md#purity-not-secured) — a limit
   already stated, of a kind with `G4` being the affine approximation of
   exactly-once. It belongs in the verification boundary, not in a blocker list.

So the ledger keeps `gate-faithful` parked, with the blocker restated once
more: not "the token is unbound" — fixed by G13 — not purity — closed on
advisory input — and no longer the returned value, which Q12's R2 closed by
derivation. What is left is **(2) the one remaining unimplemented gate**.
`gate-faithful` quantifies over *every* operation of an algebra, and an algebra
with a conditional operation cannot be declared here at all, so it cannot state
this proposition. That is now pinned by a runnable case rather than by this
paragraph: `gate_markers.rs::a_conditional_marker_has_a_signature` asks the
macro to accept the marker, and deleting its `#[ignore]` reports whether it
does.

The row that *did* move is `returned-value-unconstrained`, from `parked` to
`enforced`, on
`gate_markers.rs::the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render`
— a judgmental body that returns the argument it was handed, refused by the
injected epilogue, and red when that injected call is deleted.

### Received advisory input

An outside reader with no standing over this document returned an analysis
proposing to collapse all three blockers. In Het's own terms that is a
**reason**, not a remedy
([`reason-is-not-an-edit`](../../../docs/rung-het-props.md#reason-is-not-an-edit)):
stating why a position fails is classification, supplying the replacement is
authorship. It is recorded here and acted on where it holds.

**On blocker (3) — accepted, and it closes.** $\eta$ is $\mathcal{P}$'s unit;
factoring through it is $\mathcal{P}$-purity by construction. Q11 had read the
phrase as absolute purity, which turned an already-stated limit into an
apparent blocker. Folded above.

**On blocker (2) — half accepted.** The strong half is that a declaration
carrying bodies *is a model*, and a model must take a stand on a conditional
gate rather than defer it. That has more support than the reader knew:
[`freeness-enforced-only-with-bodies`](../../../docs/rung-ct-props.md#freeness-enforced-only-with-bodies)
already draws the theory/model line inside the macro on exactly that basis. The
weak half is the proposal to express conditionality as a branching transition,
`ConditionCheck => { DecidableOutcome | JudgmentRequired }`. That is a
per-invocation runtime branch;
[`conditional-partitions-fiber`](../../../docs/rung-het-props.md#conditional-partitions-fiber)
partitions $\mathsf{Mod}(\Sigma)$ — a static property of *which fiber a model
sits in*. A ladder that may branch either way per call sits in neither class,
which contradicts the partition rather than implementing it. And
[`classifier-one-level-up`](../../../docs/rung-het-props.md#classifier-one-level-up)
requires the classification be a *sentence* something can evaluate; "the author
chose a marker" records no classifier.

**On blocker (1) — advances, does not close.** The proposal is an *epilogue
guard*: capture $\pi(a)$ before the body consumes its argument, run the body,
then assert $\pi(f(a)) \cap \pi(a) = \emptyset$ on the way out, mirroring the
`G13` prologue. The mechanism is sound and the checked-versus-unchecked
difference is real. It does not reach the hazard, because
`Provenanced::provenance` is implemented **by the domain on its payload type**
and the body constructs the payload — so the guard reads a provenance *the body
supplies*. A body that computes internally and stamps its output with the
judge's tag passes.

The proposal quotes the thing that would close it — a payload whose provenance
is *not freely chosen by the body* — and then does not build it. Minting the
output provenance from the token rather than reading it from the returned value
is what makes the epilogue sound, and that is
[Q12](q12-admissibility-value-or-dispatch.md)'s **R2**. The useful consequence:
R1 and R2 are not symmetric options. The epilogue only works in R2's world.

The `G8` analogy also does not carry. `must_progress` compares body-produced
payloads to detect a *stall* — a liveness property, where a lying body only
harms itself. Admissibility is a safety property against a body with motive.
Same mechanism, different threat model.

**What it does not touch.** Nothing in the analysis addresses
[Q12](q12-admissibility-value-or-dispatch.md)'s decisive observation: `theory!`
emits `settle(model, q, v: Verdict)` with the verdict as a *parameter*. The
epilogue guards the `ladder!` path; the constant-arrow hazard lives on the
`theory!` path and would survive all three proposed remedies intact.

*Since resolved.* Q12's R2 removed the parameter: `settle` takes a sealed
`Judgment` minted by the principal, and there is no term through which a caller
can state a verdict. The reader's mechanism was sound and its predicate was
wrong; both halves of that assessment held.

### What would falsify this argument

Any one of these would close the question or collapse a blocker:

- **A reading of `gate-faithful` as a classification claim.** If "every
  judgmental operation is a judgmentally-admissible Kleisli arrow" is meant as
  *"is dispatched through the judgmental filter"* rather than *"lands in
  $\mathbf{Kl}_{\text{judg}}$"*, blocker (1) dissolves and the proposition is
  about gate *marking*, not gate *semantics*. The formalism as written does not
  support that reading — 5.41 gives the sub-category by its arrow condition, not
  by its dispatch discipline — but the formalism is the repo owner's to amend,
  and this is the amendment that would do it.
- ~~**A rung payload whose provenance is not freely chosen by the body.**~~
  **Done, and it closed blocker (1).** `rung::Judgment` derives $\pi$
  structurally from the judge that minted it, and the prologue trick applied on
  the way out is [G15](../../../docs/rung-props.md#g15-outcome-provenance). The
  predicate it asserts is *containment*, not the disjointness this bullet
  imagined — disjointness follows, and asserting it too would be asserting a
  conclusion.
- ~~**`#[authorial]` and `#[conditional(..)]` implemented,** which removes (2)
  outright.~~ **Half done.** `#[authorial(Role)]` is built (G14) and verified by
  mutation: dropping the capability conjunct from `Pool::authorize` reddens
  `gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one`,
  and stubbing the injected standing prologue reddens
  `::the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads`.
  What remains of this falsifier is `#[conditional(..)]`, which is hard for the
  reason given under "What makes it hard" below and is not a matter of more
  building.
- **An effect discipline on decidable bodies,** which removes (3).

Blocker (1) *was* the load-bearing one; it is closed, and the falsifier that
closed it is the second bullet above — a rung payload whose provenance is not
freely chosen by the body. It was "worth trying and has not been"; it has now
been tried, and it worked. What remains is blocker (2), and it is the one that
is not a matter of building more.

## What makes it hard

1. **Admissibility is a runtime property.** ~~Disjointness of provenance tags is
   decidable but not static; it depends on the specific principal and the
   specific argument. Pushing it into the type means a token indexed by what it
   was measured against — branding or generativity, not an ordinary generic.~~
   **Answered.** The token carries `π(a)` as a *value* and every consumer
   compares it. A brand would index the token by a scope rather than by the
   argument, force a scoped-closure API, and change every signature `ladder!`
   emits — for a check that is decidable at runtime anyway. The type does not
   have to hold the fact; it has to make the check unskippable, which is what
   the injected prologue does.
2. ~~**`authorial` needs a third signature, and standing is per-call.**~~
   **Answered, in the only way available.** The third signature exists —
   `fn t(prev, pen: Authorized<'_, R>)` — and the marker does *not* try to
   express "decidable in this model, judgmental in that one." It cannot, and it
   does not have to: standing is settled **per call** by `Pool::authorize`, at
   the point where a concrete principal and a concrete container are both in
   hand. The static marker declares only that this arrow needs a pen; which
   branch of the conditional gate the pen came through is a runtime fact the
   pen's existence records. Where the judgmental branch fires, `authorize`
   refuses and names it rather than pretending the marker settled it — so the
   difficulty is real and is surfaced, not dissolved.
3. **`conditional` has no static reading at all.** Het's fourth gate classifies
   per model, one level up
   (`rung-het-props.md#classifier-one-level-up`). rung's checks run at
   expansion time against a declaration. This is the first place Het's
   per-model classification meets rung's static checking, and the two do not
   obviously meet.
4. **Defaulting is safe but silent.** An unmarked transition reading as
   `decidable` cannot launder anything — a body needing an outside will not
   typecheck in that position. But "cannot launder" is weaker than "is
   faithful": a decidable transition may still reach a clock, a file, or a
   network, because the decidable signature excludes only *Het's* outside
   (`rung-het-props.md#purity-not-secured`).

## Why it matters

This is the largest unclosed distance between Het and rung. `conformance.md`
carries `gate-faithful` and `mod-only-gate-faithful` as `deferred` with the gap
named, and they are the only two rows whose blocker was, until now, *"no
question filed."* Three rows have since moved off `deferred` on the back of this
work — `non-identity-by-construction`, `disjointness-against-argument`,
`argument-governs` — which is the measure of how much of the distance the two
halves cover, and of how much they do not.

It also bears on every `enforced` row in that ledger. Those rows name a rung
guarantee that makes a proposition hold — but no Het theory is expressed as a
ladder, so none of those guarantees currently applies to any Het code. Gate
markers are the step that makes the ledger's central column true rather than
prospective.

G14 moved six rows off `out-of-scope`/`expressible` onto `enforced`, each with a
mutation that reddens a named test:
[`one-pool-two-filters`](../../../docs/rung-het-props.md#one-pool-two-filters),
[`authorial-qualifying-set`](../../../docs/rung-het-props.md#authorial-qualifying-set),
[`judgment-refuses-authorship-requires`](../../../docs/rung-het-props.md#judgment-refuses-authorship-requires),
[`provenance-overlap-is-the-point`](../../../docs/rung-het-props.md#provenance-overlap-is-the-point),
[`authorial-declares-standing`](../../../docs/rung-het-props.md#authorial-declares-standing),
and [`standing-conditional-gated`](../../../docs/rung-het-props.md#standing-conditional-gated).
It moved `gate-faithful` not at all, which is the honest measure of the
distance: six propositions about *who may act* are now machine-checked, and the
proposition about *what an arrow returns* is exactly where it was.

## Relationship to other questions

- **Q1 (transition-body correctness)** — the boundary this runs into. rung
  proves an arrow was traversed, never what its body computed. Admissibility is
  a body property, so any answer that leaves it to the body inherits Q1's
  limit whole.
- **Q7 (resolved)** — settled that effects layer on the forward pass of the
  Prism. A gate marker is a *classification* of that forward pass, so Q7's
  frame is the one to build in.
- **Q8 (async driver)** — a judgmental transition consults an outside, which is
  where `.await` lands. Independent of this question, but the two meet in the
  same signature.

## Most promising angle

Split the two halves and close them separately rather than looking for one
mechanism.

1. ~~Marker on the transition; emit the distinguishing signature; prove
   mis-marking is a compile error with a `compile_fail` that is verified to
   fail for the intended reason and not incidentally.~~ **Done** — G12, with
   `trybuild` snapshots rather than `compile_fail` doctests, because rustdoc
   does not verify error codes.
2. ~~Bind the qualifying token to its argument, then show that an arrow
   declared judgmental cannot **return** a value whose provenance meets its
   argument's.~~ **Both done.** G13 binds the token (verified by mutation:
   removing the `admit` call reddens two named tests, and stubbing the injected
   prologue reddens a third whose ladder body never reads its token). G15 and
   `settle` close the return (verified by mutation: deleting the injected
   epilogue call reddens
   `::the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render`;
   deleting `settle`'s containment assertion reddens
   `gate_law.rs::a_judgment_rendered_by_another_principal_is_refused`; minting
   the token's `Judgment` with the argument's provenance instead of the judge's
   reddens `::a_judgmental_arrow_may_not_return_the_provenance_it_judged`). The
   live work is step 3's residue and blocker (2).
3. ~~Only then ask whether (1) + (2) is what Het means by gate-faithful.~~
   **Asked and answered: no.** See "Does half one + half two = gate-faithfulness?"
   above, with its falsifiers. The question now stays open on **one** thing —
   the unimplemented conditional gate — plus the narrowed residue of step 2's
   second clause: the authorial gate's outward condition, and what a *branching*
   judgmental transition's arms are, admissibility-wise.
