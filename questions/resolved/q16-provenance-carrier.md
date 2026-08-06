---
id: q16
status: resolved
depends_on:
  - {on: q14, kind: premise}
  - {on: q16-ruling-commission-record, kind: evidence}
affects:
  - {target: bootstrap-stage-5, kind: gate}
  - {target: q17, kind: premise}
---

# Q16 — What carrier makes `authored(p)` a derived fact? *(resolved)*

**Status:** RESOLVED (2026-08-05) · **Ruling:** the carrier is a **commission
contribution record** — `authored(p) = ⋃_{c∈S} C(f,c)`. The two forbidden shapes
are refused; the implementation of the record is the open follow-on.

**Question.** Q14's ruling adopted the stake-based provenance map
`π(p) = authored(p) ∪ {id(p)}` with `authored(p)` the commission-local set of
artifacts in which the principal has stake — and explicitly declined to supply a
**carrier**: a source the pool can actually evaluate. Today the pool reads only
the static `PrincipalSpec::authored` field in `population.yaml`, which stays
empty for every model as an acknowledged, temporary placeholder (not the
per-invocation reading — a test pins it so it cannot ship as a working
configuration).

> **What makes `authored(p)` a derived fact rather than a static declaration?**

## Why this is not the same question as Q14

Q14 asked *what π must mean*. That is a definition, and it is settled. This asks
*where the values come from* — a different, smaller, and implementation-shaped
question. Q14 could be closed without a carrier; Stage 5 cannot be unblocked
without one, because a model judge whose `authored` is empty still qualifies
vacuously under the only provenance the pool can read.

## What a carrier must supply

To be acceptable, the carrier must let the pool produce `authored(p)` for a
model principal **and meet the same three conditions Q14 required**:

1. **Decidable at qualification time** — finite facts the pool possesses at
   dispatch, with no network call or judgment hiding in the provenance itself.
2. **Non-vacuous** — inside a commission, a model cannot judge any artifact it
   (or another instance of the same family) produced under that commission.
3. **Not total** — closed commissions' artifacts remain open to later, disjoint
   commissions; the corpus is not permanently poisoned.

## What it cannot be

- **A guessed static list in `population.yaml`.** The actual contribution is
  collective and untracked; asserting entries we do not know manufactures the
  exact provenance tags the floor exists to protect (`population.yaml` warns of
  this directly). This is Q14's (B), refused at the meta-level.
- **A source that needs its own judgment to run.** If computing `authored(p)`
  itself required a principal, the regress would re-open what the ruling closed.

## Shapes a carrier might take

- **A commission log.** The harness records, per commission, which artifact(s)
  each principal produced; `authored(p)` is read from the log rather than typed.
  A new commission starts empty; prior commissions carry forward only if the
  supplier says so. This matches Q14's ruling clause-for-clause and is the
  natural reading of "harness state the pool reads."
- **Per-artifact attribution.** Each artifact carries its own `authored` tag
  naming the family that produced it; `authored(p)` is the union of tags over
  artifacts it is responsible for. Same honesty, different anchoring.
- **A ruling that no carrier exists yet and Stage 5 waits.** Coherent and
  honest — the judgmental fragment stays declared-and-unsettled until someone
  builds the log. Acceptable; it is the "gated by cost, not mechanism" outcome.

## What rests on it

Stage 5 of the bootstrap — a model as judge — is gated on a real carrier
outright. Q14 cleared the definitional gate; this is the implementation gate
that replaced it. Until a carrier exists, the pool's model principals are
capable of the questions roles but cannot be *meaningfully* dispatched under P0,
and `population.yaml` must not be hand-populated.

## Relation to neighbours

- **[Q14](../resolved/q14-model-principal-provenance.md)** — the premise, above.
  Q14 fixed the map; this supplies the values. Q14's own State records this as
  its follow-on obligation.
- **[Q2](../parked/q2-cross-crate-provenance.md)** — unrelated despite the
  shared word. Q2 asks whether a *token* survives a crate boundary; this asks
  where a *principal's* stake is recorded.
- **[Q17](../resolved/q17-provenance-carrier-implementation.md)** — the follow-on,
  below, now resolved with it. Q16's ruling settles the definition; Q17 built
  the record `C` and wired it into the pool. Q16 is resolved because its owed
  work moved to Q17 — a scanned question, not a dead note — and Q17 is resolved
  because that work is now done.

## State

- **2026-08-05** — Filed. Raised by Q14's resolution: the ruling adopted
  stake-based π and explicitly left the carrier open. Filed as its own question
  rather than folded into Q14's body, because it has a distinct resolution
  condition and its own gate on Stage 5 — a bet that would otherwise evaporate
  inside a resolved question's State entry.
- **2026-08-05** — Resolved (definitional). An outside expert's ruling adopts
  the **commission contribution record** as the carrier: `authored(p) =
  ⋃_{c∈S} C(f,c)`, with `C : Family × CommissionId → P_fin(ArtifactId)`,
  commission boundaries harness state, and prior commissions entering `S` only
  by explicit supplier decision. The two forbidden shapes (a guessed static
  list; a source needing its own judgment) are refused. Relocating a question
  to `resolved/` is refused without an evidence edge, so the ruling is written
  as `questions/resolved/_evidence/q16-ruling-commission-record.md`.
  **The implementation is open.** The record `C` does not exist yet and is not
  wired into the pool, so model principals still read the static empty field
  and remain open-as-recorded under P0 until a commission attributes work; the
  definitional gate is gone and only the record's population is left. The
  implementation was filed as Q17, where it is scanned, and is now resolved
  with the carrier built.
