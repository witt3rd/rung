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

**Question.** Het requires every algebra in `Mod(Σ)` to be **gate-faithful**
([`gate-faithful`](../../rung-het-props.md#gate-faithful)): every decidable operation factors
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
[`non-identity-before-dispatch`](../../rung-het-props.md#non-identity-before-dispatch)
and [`non-identity-by-construction`](../../rung-het-props.md#non-identity-by-construction)
now hold as type-and-runtime facts rather than as caller discipline. No
judgmental arrow can be traversed except by a principal drawn from
$\mathcal{P}_{\text{judg}}(\varphi, a)$ for the very $a$ it is applied to.

**Why that is not gate-faithfulness.**

1. **Admissibility constrains the arrow's *output*, and this constrains its
   *input*.** [`gate-faithful`](../../rung-het-props.md#gate-faithful) is
   stated through
   [`admissibility-subcategories`](../../rung-het-props.md#admissibility-subcategories),
   which defines the judgmental sub-category as
   $\mathbf{Kl}_{\text{judg}}(\mathcal{P}) = \{ f : \pi(f(a)) \cap \pi(a) = \emptyset \}$.
   That is a condition on $f(a)$ — the value returned. G12 and G13 together
   settle *who may be consulted about $a$*; they say nothing about the
   provenance of what comes back. A judgmental transition whose body returns a
   rung carrying $\pi(a)$ itself is inadmissible and passes every check built
   here. That is Q1's territory, and this question inherits it whole — which is
   exactly what the "Relationship to other questions" section warned.

2. **One of the four gates has no encoding at all** — down from two.
   `#[authorial(Role)]` is implemented (rung-props.md G14): it emits an
   `Authorized<'_, Role>` pen, `Pool::authorize` runs **both** conjuncts of
   [`authorial-qualifying-set`](../../rung-het-props.md#authorial-qualifying-set),
   and a macro-injected prologue admits the pen only over the container the
   subject sits in. So an algebra with an authorial operation can now state the
   property — for that operation's *input*. `#[conditional(..)]` remains a
   parse-time refusal, and gate-faithfulness quantifies over *every* operation,
   so an algebra with a conditional operation still cannot state it here at all.

   Two things this did **not** do, and they are why the blocker is half-closed
   rather than closed. It did not touch blocker (1): the authorial half of
   [`admissibility-subcategories`](../../rung-het-props.md#admissibility-subcategories)
   is `π(f(a)) ⊆ π(p) ∧ standing(p, a)`, and G14 secures the standing conjunct
   on the way *in* while leaving the containment conjunct on the way *out*
   entirely to the body — the same shape as G13's gap, on the second gate.
   `Prov::contained_in` exists and no guarantee calls it. And it did not close
   the *conditional* branch of
   [`standing-conditional-gated`](../../rung-het-props.md#standing-conditional-gated):
   where provenance containment does not settle standing,
   `Pool::authorize` returns `AuthorizeError::StandingIsJudgmental` rather than
   minting a pen. That refusal is honest and is tested, but Het says a judge
   rules there
   ([`standing-terminates-at-depth-one`](../../rung-het-props.md#standing-terminates-at-depth-one)),
   and rung has no term for that dispatch.

3. **`decidable` still does not factor through $\eta$.** The unmarked signature
   excludes $\mathcal{P}$ and nothing else
   ([`purity-not-secured`](../../rung-het-props.md#purity-not-secured)). A
   decidable transition may read a clock. "Factors through $\eta$" is strictly
   stronger than "cannot reach the pool."

So the ledger keeps `gate-faithful` as `deferred`, with the blocker restated:
not "the token is unbound" — that is fixed — but (1) the returned value, (2) the
one remaining unimplemented gate, (3) purity. Blocker (2) was two gates and is
now one; blockers (1) and (3) are untouched, and (1) got *wider*, because there
are now two admissibility conditions on the way out rather than one.

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
- **A rung payload whose provenance is not freely chosen by the body.** If a
  judgmental transition's target payload derived $\pi$ structurally rather than
  by the body's construction, blocker (1) becomes checkable by the same
  prologue trick applied on the way out. This is worth trying and has not been.
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

Blocker (1) is the load-bearing one, and it is the one that is not a matter of
building more.

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
[`one-pool-two-filters`](../../rung-het-props.md#one-pool-two-filters),
[`authorial-qualifying-set`](../../rung-het-props.md#authorial-qualifying-set),
[`judgment-refuses-authorship-requires`](../../rung-het-props.md#judgment-refuses-authorship-requires),
[`provenance-overlap-is-the-point`](../../rung-het-props.md#provenance-overlap-is-the-point),
[`authorial-declares-standing`](../../rung-het-props.md#authorial-declares-standing),
and [`standing-conditional-gated`](../../rung-het-props.md#standing-conditional-gated).
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
2. Bind the qualifying token to its argument, ~~then~~ **done** (G13, verified by
   mutation: removing the `admit` call reddens two named tests, and stubbing the
   injected prologue reddens a third whose ladder body never reads its token).
   What remains of this step is the second clause, untouched: *show that an
   arrow declared judgmental cannot **return** a value whose provenance meets
   its argument's.* That is the live work.
3. ~~Only then ask whether (1) + (2) is what Het means by gate-faithful.~~
   **Asked and answered: no.** See "Does half one + half two = gate-faithfulness?"
   above, with its falsifiers. The question stays open on step 2's second clause
   — now doubled, since the authorial gate has an outward condition of its own
   — and on the one remaining unimplemented gate.
