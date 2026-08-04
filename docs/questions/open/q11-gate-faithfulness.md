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
| the signature is honest | a gate marker; a decidable position admits no principal parameter | design agreed, unbuilt |
| the arrow is admissible | the qualifying token is constructible only by the filter, and is **bound** to the argument it was measured against | open — the token is currently unbound |

The conjunction may or may not equal gate-faithfulness. Nobody has argued that
it does.

## What makes it hard

1. **Admissibility is a runtime property.** Disjointness of provenance tags is
   decidable but not static; it depends on the specific principal and the
   specific argument. Pushing it into the type means a token indexed by what it
   was measured against — branding or generativity, not an ordinary generic.
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
question filed."*

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

1. Marker on the transition; emit the distinguishing signature; prove
   mis-marking is a compile error with a `compile_fail` that is verified to
   fail for the intended reason and not incidentally.
2. Bind the qualifying token to its argument, then show — by mutation, not by
   inspection — that an arrow declared judgmental cannot return a value whose
   provenance meets its argument's.
3. Only then ask whether (1) + (2) is what Het means by gate-faithful, or
   whether something is still missing. Argue it; do not assume it.
