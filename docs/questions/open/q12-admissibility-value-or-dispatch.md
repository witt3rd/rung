---
id: q12
status: open
depends_on:
  - {on: q11, kind: premise}
---

# Q12 — Does admissibility constrain the value, or the dispatch? *(open)*

**Status:** OPEN · **Gated:** the ruling is the repo owner's.

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
([`reason-is-not-an-edit`](../../rung-het-props.md#reason-is-not-an-edit)): it
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
