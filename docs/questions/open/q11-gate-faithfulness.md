---
id: q11
status: open
depends_on:
  - {on: q1, kind: justification}
---

# Q11 — Gate-faithfulness *(open)*

**Status:** OPEN

**Question.** Het requires every algebra in `Mod(Σ)` to be **gate-faithful**
([`gate-faithful`](../rung-het-propositions.md#gate-faithful)): every decidable operation factors
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
(`rung-het-propositions.md#mismarking-is-not-a-false-claim`). The open part is
that **gate-faithfulness as Het states it is a property of the interpretation,
not of the declaration.**

Admissibility is a condition on provenance at the point of use —
`π(f(a)) ∩ π(a) = ∅` — which is about the values an arrow actually returns, not
about its type. So a marker makes the *signature* honest. It does not make the
*arrow* admissible. Two halves:

| half | what would close it | status |
|---|---|---|
| the signature is honest | a gate marker; a decidable position admits no principal parameter | **built** — SPEC.md G12 |
| the arrow is admissible | the qualifying token is constructible only by the filter, and is **bound** to the argument it was measured against | **built** — SPEC.md G13 |

Both halves are now built. The conjunction is **not** gate-faithfulness. The
argument is below.

## Does half one + half two = gate-faithfulness? No.

This section discharges the standing complaint that nobody had argued it either
way. The answer is no, and the reason is that the table above mislabels its
second row: what G13 delivers is not "the arrow is admissible."

**What was built.** `Qualified<R>` records the principal *and* `π(a)`, the
provenance of the argument disjointness was measured against. `Qualified::admit`
is the only gate that spends it, and every consumer runs it: `dispose` against
the Proposal, `theory!`'s `settle` against the model, and — for a
`ladder!` transition — a macro-injected prologue the domain's body cannot skip,
on the G8 `must_progress` precedent. So a licence earned honestly against one
argument is refused everywhere else.

**What that is.** It is P0, completely:
[`non-identity-before-dispatch`](../rung-het-propositions.md#non-identity-before-dispatch)
and [`non-identity-by-construction`](../rung-het-propositions.md#non-identity-by-construction)
now hold as type-and-runtime facts rather than as caller discipline. No
judgmental arrow can be traversed except by a principal drawn from
$\mathcal{P}_{\text{judg}}(\varphi, a)$ for the very $a$ it is applied to.

**Why that is not gate-faithfulness.**

1. **Admissibility constrains the arrow's *output*, and this constrains its
   *input*.** [`gate-faithful`](../rung-het-propositions.md#gate-faithful) is
   stated through
   [`admissibility-subcategories`](../rung-het-propositions.md#admissibility-subcategories),
   which defines the judgmental sub-category as
   $\mathbf{Kl}_{\text{judg}}(\mathcal{P}) = \{ f : \pi(f(a)) \cap \pi(a) = \emptyset \}$.
   That is a condition on $f(a)$ — the value returned. G12 and G13 together
   settle *who may be consulted about $a$*; they say nothing about the
   provenance of what comes back. A judgmental transition whose body returns a
   rung carrying $\pi(a)$ itself is inadmissible and passes every check built
   here. That is Q1's territory, and this question inherits it whole — which is
   exactly what the "Relationship to other questions" section warned.

2. **Two of the four gates have no encoding at all.** `#[authorial]` and
   `#[conditional(..)]` are parse-time refusals. Gate-faithfulness quantifies
   over *every* operation of an algebra, so an algebra with an authorial
   operation cannot state the property here, let alone satisfy it. The `enact`
   path in `rung-het` takes an `Authorized` pen, but it is hand-rolled: no
   marker emits it, and nothing checks that an authorial arrow is the only place
   it appears.

3. **`decidable` still does not factor through $\eta$.** The unmarked signature
   excludes $\mathcal{P}$ and nothing else
   ([`purity-not-secured`](../rung-het-propositions.md#purity-not-secured)). A
   decidable transition may read a clock. "Factors through $\eta$" is strictly
   stronger than "cannot reach the pool."

So the ledger keeps `gate-faithful` as `deferred`, with the blocker restated:
not "the token is unbound" — that is fixed — but (1) the returned value, (2) the
two unimplemented gates, (3) purity.

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
- **`#[authorial]` and `#[conditional(..)]` implemented,** which removes (2)
  outright.
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
2. **`authorial` needs a third signature, and standing is per-call.** Standing is
   conditional-gated (`rung-het-propositions.md#standing-conditional-gated`):
   decidable where provenance containment settles it, judgmental otherwise. A
   static marker cannot express "decidable in this model, judgmental in that
   one."
3. **`conditional` has no static reading at all.** Het's fourth gate classifies
   per model, one level up
   (`rung-het-propositions.md#classifier-one-level-up`). rung's checks run at
   expansion time against a declaration. This is the first place Het's
   per-model classification meets rung's static checking, and the two do not
   obviously meet.
4. **Defaulting is safe but silent.** An unmarked transition reading as
   `decidable` cannot launder anything — a body needing an outside will not
   typecheck in that position. But "cannot launder" is weaker than "is
   faithful": a decidable transition may still reach a clock, a file, or a
   network, because the decidable signature excludes only *Het's* outside
   (`rung-het-propositions.md#purity-not-secured`).

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
   and on the two unimplemented gates.
