---
id: q17
status: open
depends_on:
  - {on: q16, kind: premise}
affects:
  - {target: bootstrap-stage-5, kind: gate}
---

# Q17 — Implement the commission contribution record (the carrier) *(open)*

**Status:** OPEN

**Question.** Q16's ruling resolved the *definition* of the carrier: it is a
**commission contribution record** — a finite map

$$C:\mathsf{Family}\times\mathsf{CommissionId}\to\mathcal{P}_{\mathrm{fin}}(\mathsf{ArtifactId}),$$

with `authored(p) = ⋃_{c∈S} C(f,c)` for the active commission set `S`. This
question asks for the *implementation* of that record and its wiring into the
pool — the work Q16's ruling explicitly left open.

> **Where does the commission record `C` live, who writes it, and how does the pool read `authored(p)` from it at qualification time?**

## Why this is not Q16

Q16 asked what the carrier *must be*; that is a definition, and an outside
ruling settled it. This asks how the harness **maintains and reads** the record.
Q16 could close with the record unimplemented, as it did; this question cannot
close until the pool's `authored(p)` for a model principal is genuinely derived
from `C` and `S` rather than read from the static, empty placeholder in
`population.yaml`.

## The interface the ruling already specifies

The pool must be able to evaluate, at qualification time and with no network
call and no judgmental sentence in the provenance itself:

- `C(f,c)` — the finite set of artifacts family `f` produced under commission
  `c`;
- `S` — the active commission set for the dispatch: the current commission plus
  any prior commissions the supplier **explicitly** carried forward, with a new
  commission starting at `∅` for every family and prior commissions **never**
  entering `S` automatically.

Disjointness is then ordinary finite-set comparison against the argument's
`authored`.

## What counts as done

1. A record type (`CommissionLog` or equivalent) exists and is typed, not a
   stringly map, so a typo'd family or artifact cannot silently empty a set.
2. The pool reads `authored(p)` for a model principal from the record at
   qualification, not from the static `PrincipalSpec::authored` field.
3. The three Q14 conditions are exercised by a test, not asserted:
   **decidable** at qualification, **non-vacuous** inside an open commission,
   and **not total** across closed, non-carried-forward commissions.
4. The `population.yaml` placeholder, and the test pin that keeps it from
   shipping as a working configuration, are retired only when the record is the
   real source — not before.

## What it cannot be

- **A guessed static list** in `population.yaml` or anywhere else. The actual
  contribution is collective and untracked; asserting entries we do not know
  manufactures the provenance tags the floor exists to protect. This is the
  ruling's meta-level refusal, carried forward.
- **A source that needs its own judgment to run.** Computing `authored(p)` must
  not itself require a principal, or the regress Q16's ruling closed re-opens.
- **A half-record with a made-up backfill.** The log starts **empty** for a new
  commission; it is filled by the harness recording work as it happens, not by
  retroactively claiming artifacts we cannot attribute.

## What rests on it

Stage 5 of the bootstrap — a model as judge — is gated on a real carrier
outright. Q14 cleared the definitional gate and Q16 cleared the carrier's
definition; this is the implementation gate that remains. Until `C` exists and
is wired in, the pool's model principals are capable of the questions roles but
cannot be *meaningfully* dispatched under P0, and `population.yaml` must not be
hand-populated.

## Relation to neighbours

- **[Q16](../resolved/q16-provenance-carrier.md)** — the premise, above. Q16
  fixed the carrier; this builds it. Q16's State and this file's existence are
  what keep Q16's `resolved/` home honest — no owed work hides in a done-pile.
- **[Q14](../resolved/q14-model-principal-provenance.md)** — Q16's premise, and
  once removed. Q14 fixed the map; Q16 fixed the carrier's definition; this
  supplies the values the pool can actually read.
- **[Q2](../parked/q2-cross-crate-provenance.md)** — unrelated despite the
  shared word. Q2 asks whether a *token* survives a crate boundary; this builds
  where a *principal's* stake is recorded.

## State

- **2026-08-05** — Filed. Raised by Q16's resolution: the ruling adopted the
  commission contribution record and explicitly left "the implementation of the
  record" open. Filed as its own question rather than folded into Q16's body for
  the same reason Q16 was filed rather than folded into Q14's — a distinct
  resolution condition and its own gate on Stage 5, which would otherwise
  evaporate inside a resolved question's State entry.
