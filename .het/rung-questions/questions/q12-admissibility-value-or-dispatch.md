---
id: q12
status: resolved
depends_on:
  - {on: q11, kind: premise}
affects:
  - {target: q14, kind: premise}
---

# Q12 — Does admissibility constrain the value, or the dispatch? *(resolved)*

**Status:** RESOLVED (2026-08-04) · **Ruling: R2 — adopt verdict provenance.**
The value reading is load-bearing and stands. A judgmental arrow's outcome
carries its judge's provenance; the outside supplies the verdict; and output
admissibility then *derives* rather than needing a check of its own.

## The ruling

R2, as filed below, with one thing the filing did not anticipate.

**The oracle.** `Principal` gains `rule(matter) -> Verdict`. Observation (2)
below is exactly that no method of `Principal` returned one — so the audit's
decisive point is answered at the trait, not at the call site.

**The seal.** `Judgment` is a verdict together with `π(p)`, sealed as
`Qualified` and `Authorized` are (rung-props.md G2): no constructor outside
`rung`, and `Principal::judgment` — which calls the oracle — is the only mint.
The model cannot mint one; nor can a token, because copying
`Qualified::principal_provenance` onto a locally computed value produces a
*claim* that a judge ruled and not a judge's ruling. `Pool::consult` qualifies
and asks in one act, so the licence and the judgment are the same principal's
by construction.

**The chain.** `π(f(a)) ⊆ π(p)` is asserted wherever a `Judgment` is spent:
`theory!`'s `settle` refuses `SettleError::OutcomeNotFromJudge`, and `ladder!`
injects `must_derive_from_judge` as an **epilogue** on a forward judgmental
transition (rung-props.md G15) — the mirror of G13's prologue, on the way out.

**The derivation, which is the part that changes what had to be built.** G13
already enforces `π(p) ∩ π(a) = ∅` for the very argument the arrow is applied
to. So

$$\pi(f(a)) \subseteq \pi(p) \ \wedge\ \pi(p) \cap \pi(a) = \emptyset \implies \pi(f(a)) \cap \pi(a) = \emptyset$$

and `admissibility-subcategories`' judgmental clause is a **theorem of two
enforced facts** rather than a third guarantee. Nothing calls `Prov::overlaps`
on the way out, and nothing should: an epilogue asserting the conclusion of a
derivation whose premises are both enforced reads as a guarantee and is none.
This is why the received advisory input's epilogue proposal was right about the
mechanism and wrong about the predicate — the guard that works is the
containment one, and it works only in R2's world, which is what that input
itself concluded.

**Where the derivation does *not* carry, stated exactly.** It carries wherever
containment is enforced, and containment is enforced in two places: `settle`,
and forward judgmental `ladder!` transitions. It does **not** carry for

- the **authorial** outward conjunct. `admissibility-subcategories` states it
  as `π(f(a)) ⊆ π(p) ∧ standing(p, a)`; G14 secures `standing` on the way in
  and leaves containment on the way out to the body. R2 is the judgmental
  mirror of `proposal-provenance-is-authors` and does not reach it.
- **branching** judgmental transitions. A branching outcome is a sum whose
  recoverable and continue arms carry the argument onward *by design* —
  re-entry, not laundering (`reproposal-carries-the-chain`,
  `no-bound-on-reentry`) — so which arms are *outcomes* in 5.41's sense is
  unsettled, and an epilogue there would refuse the re-entry rather than the
  hazard. This is a question, not a hole, and it is not settled here.

Both are recorded as `rung-props.md#outward-conditions-remaining` and parked on
`gate_markers.rs::an_authorial_arrow_may_not_return_a_provenance_its_author_does_not_hold`,
whose `#[ignore]` reason names them.

**`constant-arrow-hazard` is not retired.** Under R1 it would have had to be —
that was the filing's admissibility condition on R1 — and under R2 it stays
standing and is now *closed at the term level*: there is no way to write
`c_j : a ↦ η(j)` with `j` from `M`'s carrier and have it typecheck in a
judgmental position, because the outcome must be a `Judgment` and a `Judgment`
comes from a principal.

## What landed, and where

| surface | what changed |
|---|---|
| `rung-het-props.md` | **new** `judgment-provenance-is-the-judges` (5.42) — the judgmental mirror of `proposal-provenance-is-authors`, with the derivation stated beneath it. **new** `principal-provenance-floor` (3.25) — see below. |
| `rung-props.md` | **new** `G15` outcome provenance; `3.721` the judgmental outcome bound; `5.6`/`5.62` restated, `5.621` names the residue. |
| `rung` | `Principal::{authored, rule, judgment}`, `Judgment`, `OutcomeNotFromJudge`, `SettleError`, `Pool::consult`, `Prov::{with, contains}`; `Settled::Judgmental` carries the sealed `Judgment` rather than a bare `Verdict`. |
| `rung-macro` | the injected epilogue and `must_derive_from_judge`. |
| `conformance.md` | `returned-value-unconstrained` moves `parked → enforced`; `outward-conditions-remaining` is new and `parked`. `gate-faithful` and `mod-only-gate-faithful` stay `parked` on blocker (2). |

## The entanglement, and how it was settled

The last section below records that this question was entangled with the
empty-provenance ruling: disposing of it requires a judge disjoint from the
proposal's author, and a principal with no history declares `π(p) = ∅`, which
is disjoint from everything. That was settled **by the floor, not by admitting
∅**.

`π(p) ⊇ {id(p)}`: a principal's provenance contains its own identity.
`Principal` no longer has `Provenanced` as a supertrait and cannot state its own
`π`; it declares `authored()` — which MAY be empty — and the sole route to
`Provenanced` is the blanket impl `authored().with(id())`. So a principal with
no history in this repository is still not disjoint from itself, the universal
judge is **underivable** rather than refused, and a hand-written
`impl Provenanced for SomePrincipal` is an E0119 coherence error
(`rung/tests/ui/floor_forged_provenance.rs`).

That is the difference between the floor and the guard the entanglement seemed
to call for. A guard on `Pool::qualify` would have refused an empty `π(p)` at
the point of use and been one uncalled code path away from vacuous — the exact
failure `Qualified`'s seal exists to foreclose. A value the language will not
produce cannot be reached by any path at all.

Recorded as `rung-het-props.md#principal-provenance-floor`, pinned by
`rung/tests/provenance_floor.rs`.

## What this did to Q11

Blocker (1) **closes**, and it closes *by derivation* rather than by an epilogue
guard on the stated condition. Blocker (2) **stands**: `#[conditional(..)]` is
still a parse-time refusal and gate-faithfulness quantifies over every
operation. Q11 stays **open**, and `gate-faithful` and `mod-only-gate-faithful`
stay `parked` — on one blocker now instead of two.

---

*Everything below is the question as filed, kept whole. It is the reasoning the
ruling was made against, not a summary of the ruling.*

---

**Filed as:** OPEN · **Gated:** the ruling is the repo owner's.

**Question.** `admissibility-subcategories` defines the judgmental sub-category
as

$$\mathbf{Kl}_{\text{judg}}(\mathcal{P}) = \{\, f : \pi(f(a)) \cap \pi(a) = \emptyset \,\}$$

a condition on **the value the arrow returns**. Every mechanism built to date —
the gate marker (`G12`), the bound token (`G13`), the authorial pen (`G14`) —
constrains **who may be consulted about `a`**. Those are different claims.

**Is the returned-value form load-bearing, or is it a stronger statement of a
dispatch discipline that was always what was meant?**

## Why it is open

This is not a gap between doctrine and code. It is a gap between two readings of
one proposition, and the whole of `gate-faithful` turns on which is intended.

| reading | the claim | status |
|---|---|---|
| **value** | a judgmental arrow's output is provenance-disjoint from its input | stated; unenforced; unenforceable without a further proposition |
| **dispatch** | a judgmental operation is settled by a principal drawn through the judgmental filter | enforced today by `G12`/`G13`/`G14` |

Under the dispatch reading, `gate-faithful` becomes closable and Q11's
load-bearing blocker dissolves. Under the value reading, it stays open until a
verdict is obliged to carry the provenance of the principal that rendered it.

## The audit

Three observations, each checkable, none requiring a ruling:

1. **Nothing reads the outward condition.** `Prov::contained_in` exists in
   `rung` and no guarantee calls it. `admissibility-subcategories` and the
   authorial mirror in `authorial-admissibility-stronger` both state conditions
   on `f(a)`; neither is enforced anywhere.
2. **The hazard the condition exists to block is live.** `theory!` emits
   `settle(model, q: Qualified<R>, v: Verdict)` — **the verdict is a
   parameter**. No method of `Principal` returns a `Verdict`; the trait declares
   `capable` and `id`. The only `-> Verdict` in the workspace is on the
   *decidable* gate. A caller may compute a verdict from the model's own field
   and hand it in, and the receipt will name a judge that was never asked.
3. **An inadmissible arrow has been passing since the gate markers landed.**
   The `Review` ladder in `rung/tests/gate_markers.rs` returns a rung carrying
   the provenance it judged. It satisfies every check the workspace performs.
   The parked test at `gate_markers.rs` needed no rigging to demonstrate this —
   it was already true.

## What makes (2) decisive

`constant-arrow-hazard` names the attack the condition was written against: an
algebra sending a judgmental operation to a **constant** arrow
$c_j : a \mapsto \eta(j)$ whose value $j$ is drawn from $M$'s own carrier. The
selection rule never fires; self-reference is hard-coded into the
interpretation.

`settle(model, token, verdict)`, with the verdict computed by the caller from
the model, **is that arrow**. The returned-value form of
`admissibility-subcategories` is stated on $f(a)$ *because* the hazard is that
$f(a)$ comes from inside $M$. A dispatch discipline does not block it: an
implementation may filter its principals perfectly and still return a value it
computed itself.

So the dispatch reading does not merely weaken the proposition. It removes the
condition that blocks the hazard `constant-arrow-hazard` names, while leaving
that proposition standing. Anything that closes `gate-faithful` by adopting it
must also account for 5.4, or retire it.

## The candidate remedies

Authorial. Whoever holds standing over `rung-het-props.md` authors; a judge
disjoint from that author disposes.

**R1 — restate to the dispatch reading.** `admissibility-subcategories` becomes
a condition on which principal settles the operation. `gate-faithful` closes on
what is built. `constant-arrow-hazard` must then be retired or restated, since
nothing would block it. The parked test is *retired*, not passed.

**R2 — adopt verdict provenance.** Add the judgmental mirror of
`proposal-provenance-is-authors`: a judgmental arrow's outcome carries its
judge's provenance, $\pi(f(a)) \subseteq \pi(p)$. This obliges the verdict to
come from the principal, closing the second half of the audit above. Cost:
`Verdict` carries $\pi$, which reaches every sentence and every `settle` call,
and `Principal` gains a method that returns one.

**R3 — dispute.** The propositions are correct as written and only the
implementation is wrong. Then nothing in the formalism changes and the work is
entirely in `rung`.

A remedy that adopts R1 *and* keeps `constant-arrow-hazard` is not admissible as
stated — it would close `gate-faithful` on the weaker property while the hazard
the proposition exists to refuse stays open under its own name.

## Received advisory input (Q11)

An outside reader proposed an **epilogue guard** for blocker (1): capture
$\pi(a)$ before the body consumes its argument, run the body, then assert
$\pi(f(a)) \cap \pi(a) = \emptyset$ on the way out — the `G13` prologue
mirrored. Assessed in full under Q11's *Received advisory input*; two
consequences land here.

**It sharpens R1 versus R2 rather than sitting beside them.**
`Provenanced::provenance` is implemented by the domain on its payload type and
the body constructs the payload, so an epilogue reads a provenance *the body
supplies*. A body that computes internally and stamps the judge's tag passes it.
The guard becomes sound only when the output provenance is **minted from the
token** instead of read from the returned value — which is R2. So R1 and R2 are
not symmetric: **the epilogue only works in R2's world**, and under R1 there is
nothing for it to check.

**It leaves the decisive observation untouched.** The proposal guards the
`ladder!` path. Observation (2) above is on the `theory!` path — `settle` takes
the verdict as a parameter — and no epilogue on a transition reaches it. A
remedy that adopts the epilogue and stops there would harden the arrow while
leaving the constant arrow exactly where it is.

In Het's own terms the analysis is a **reason**, not a remedy
([`reason-is-not-an-edit`](../../../docs/rung-het-props.md#reason-is-not-an-edit)): it
holds no standing over `rung-het-props.md`, so it informs the author and does
not author. Recorded as such.

## Two things this question knows about itself

**The pass cannot close it.** Running this through the audit-rectify pass would
produce a Disposition *recorded* rather than *obtained*, because the verdict is
a parameter — which is observation (2). The machinery can carry the shape of
this end to end today and not the substance. That is not an argument against
filing it; it is the sharpest available evidence that the question is real.

**It is entangled with the empty-provenance ruling.** Disposing requires a judge
disjoint from the proposal's author. A principal with no history in this
repository declares $\pi(p) = \emptyset$, which is disjoint from everything —
and whether that is genuine disjointness or mere silence is itself a gated
question. Obtaining a judge for *this* question depends on settling *that* one.
The two were treated as independent and are not.

## What would settle it

- A ruling on which reading `admissibility-subcategories` is meant to carry.
- If R1: a restatement of `constant-arrow-hazard`, or its retirement with the
  reason recorded.
- If R2: `Principal` gains a verdict-returning method, `Settled` carries the
  judge's provenance, and the parked test at `gate_markers.rs` goes green for
  the right reason — verified by mutation, not by inspection.
