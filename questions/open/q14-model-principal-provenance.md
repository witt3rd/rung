---
id: q14
status: open
depends_on:
  - {on: q12, kind: premise}
affects:
  - {target: bootstrap-stage-5, kind: gate}
---

# Q14 — What provenance does a model principal carry? *(open)*

**Status:** OPEN

**Question.** `Kind::Llm` is an admissible principal by the theory's own
reckoning — `rung_std::principals` declares it alongside `Agent`,
`RelationalBeing` and `Human`. Non-identity (P0) requires that a judge's
provenance be disjoint from the authored artifact's: `π(p) ∩ π(a) = ∅`. So
before a model can judge anything:

> **What is `π` for a model instance?**

The provenance floor fixes that `π(p) ⊇ {id(p)}` — every principal carries at
least its own identifier, enforced structurally by the blanket
`impl<P: Principal> Provenanced for P`. That settles that `π` is non-empty. It
says nothing about **what else is in it**, and for a model the floor's own term
`id(p)` is exactly the thing that has no obvious referent.

## Why this is not the same question for a model as for a human

P0 is not a rule about names. It is a rule about **stake**: you may not rule on
your own work, because you have an interest in the outcome. For a human that
lands cleanly, because identity is continuous — the same person wrote the
document last month and is being asked to judge it today, and the continuity is
what makes the disqualification meaningful.

A model has two candidate carriers of identity and neither is that:

- **Weights**, which are shared across every instance ever served from them, and
  persist across all work.
- **Context**, which is unshared, ephemeral, and gone at the end of the call.

Neither is continuity of a being with a stake. The question is which of them, if
either, `π` should track — and the three obvious answers each fail in a
different direction.

## The three readings, and how each fails

| reading | `id(p)` is | failure |
|---|---|---|
| **per-family** | `anthropic/claude-opus-5` | Every artifact any instance of that family touched is off-limits to every instance of it. In *this* repository that is most of the corpus, including most of `docs/`. Sound, and very close to total. |
| **per-invocation** | a fresh identifier per call | Non-identity is satisfied always, and always **vacuously**. A model judges text it produced five minutes earlier because the tag differs. This is the constant-arrow hazard wearing a nonce. |
| **per-session** | the harness's context boundary | Intermediate, and incoherent: `π` would be determined by how a *caller* batched its requests. That is a fact about the orchestration, not about the principal, and P0 is a claim about principals. |

The middle row is the dangerous one, because it is the one that **passes every
check the system currently has** while making the guarantee decorative. It would
show up as a large green fragment.

## Why Q12 is the premise

Before R2, a judge's provenance was consumed at qualification and went no
further — the pool checked disjointness and the verdict was a plain value. Q12's
ruling changed that: a judgmental arrow's outcome now *carries* its judge's
provenance, and output admissibility derives from it
(`admissibility-subcategories`). So `π(judge)` no longer stops at the gate; it
travels with the verdict and enters the provenance of everything downstream.

That is what promotes this from a modelling detail to a question. Under the
per-family reading, one model judgment contaminates its output for every
subsequent model judgment. Under the per-invocation reading, nothing is ever
contaminated by anything. The choice propagates through the whole corpus rather
than being spent at a single call.

## What would count as an answer

A rule assigning `π` to a model instance that satisfies **all three** of:

1. **Decidable at qualification time** from facts the pool actually has.
2. **Does not make non-identity vacuous** — there must exist artifacts a given
   model instance cannot judge, and they must be the right ones.
3. **Does not disqualify every model from every artifact** in a corpus that
   models helped write, which is the only kind of corpus this will run on.

Any two of the three are easy. All three is the question.

### Shapes an answer might take

- **Stake-based `π`, not identity-based.** Provenance tracks what a principal
  has a stake in rather than what it *is*: a model's `π` is `authored() ∪
  {id(p)}` where `authored()` is the set of artifacts produced under the current
  commission, recorded by the harness. This is already the shape of the blanket
  impl — the open part is only the granularity of `id(p)`, and this reading says
  the floor's `{id(p)}` contributes almost nothing for a model. **A floor that
  contributes nothing for an entire admissible `Kind` is worth being uneasy
  about**, and that unease is the substance of this question rather than an
  objection to be waved off.
- **A ruling that models are never admissible judges.** Coherent, honest, and it
  voids the bootstrap proposal Stage 5 outright — the judgmental
  fragment then stays declared-and-unsettled at corpus scale indefinitely. That
  is an acceptable answer; it is not a non-answer.
- **A ruling that P0 constrains the artifact, not the principal.** Disqualify on
  `authored()` alone and let `id(p)` be per-invocation. This is probably the
  most likely resolution and it needs to be written carefully rather than
  slipped in, because it weakens the floor for exactly one `Kind` and the floor
  was introduced precisely so that no `Kind` could opt out of it.

## What rests on it

the bootstrap proposal's Stage 5 — a model as judge — is gated on
this outright, and Stage 5 is what makes the judgmental fragment tractable at
corpus scale. Without an answer, Stage 4's census of unsettled judgmental
sentences is accurate and **permanent**: a precise, unmovable record of work no
admissible principal exists to do.

It is not confined to `Kind::Llm`. `Kind::Agent` has the same discontinuity, one
level up — an orchestration's identity is its tools and its underlying models,
and neither is continuous either. Any answer should say which `Kind`s it covers.

## Relation to neighbours

- **[Q12](../resolved/q12-admissibility-value-or-dispatch.md)** — the premise,
  above. R2 is what put `π(judge)` into the verdict.
- **[Q2](../parked/q2-cross-crate-provenance.md)** — unrelated despite the
  shared word. Q2 asks whether a *token* survives a crate boundary; this asks
  what a *principal's* provenance contains in the first place.
- **[Q11](q11-gate-faithfulness.md)** — adjacent, not dependent. Q11 asks
  whether a transition is what it claims; this asks whether the principal
  settling it is who it claims. A green answer to one says nothing about the
  other.

## State

- **2026-08-04** — Filed. Raised by the bootstrap proposal (now `docs/.archive/2026-08-04_bootstrap-proposal.md`)
  §8 stage 5, which names it as an open question it raises and does not answer.
  Filed separately rather than left inline, on the reasoning that a proposal's
  own admission of a hole is exactly the thing that evaporates when the proposal
  is superseded.
