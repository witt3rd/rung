# EDGES — the typed dependency vocabulary

**2026-07-18 · normative reference for dependency edges across the registries.**

The registries (`docs/questions/`, and its siblings `outer-loop/bets/`,
`augur/genesis/meta/questions/`) are not bags of independent files. Items rest on
each other: a question is a *premise* for a doc section, a decision is *justified
by* an open question, a resolution *spawns* a successor. When one item's state
changes, some set of others must be re-examined — and **how** they must be
re-examined depends on the *kind* of edge.

This document is the vocabulary. `_reach.py` is its interpreter. The README of each
registry is the operational quickstart; this is the spec the quickstart points at.

---

## An edge is an arrow between items

Not within one. A transition (`A => B` in a ladder) is an arrow *within* a category
— rung Level 0. A dependency is an arrow *between whole items* — a Level-1 structure
over the collection of items (see `docs/questions/_map.md`, the growth tower, and
`docs/questions/open/q9-the-dependency-superstructure.md` for what that superstructure
precisely is). Each edge is **directed** and **typed**.

Edges are declared in each item's frontmatter, in two directions:

```yaml
---
id: q7
status: resolved
depends_on:                              # what THIS rests on (inbound)
  - {on: kleisli-reviews, kind: evidence}
affects:                                 # what a change to this ripples to (outbound)
  - {target: RUNG-CT§6, kind: premise}
  - {target: blocking-client-decision, kind: justification}
---
```

- `depends_on` — the things this item stands on. If one of them changes, re-examine *this*.
- `affects` — the things that rest on this item. If *this* changes, they must be reviewed.

Targets may be internal (`q8` — another registry file, recursable) or external
(`RUNG-CT§6`, `blocking-client-decision` — a doc section or a decision that cannot
carry frontmatter; surfaced as a leaf).

---

## The vocabulary — seven kinds, two axes

Each kind carries a **propagation semantics** (what a change obliges) and a
**recursion flag** (does a change ripple *through* it to *its* dependents, or stop).

| kind | when the source changes… | recurses? |
|---|---|---|
| `premise` | **MUST re-examine** — the dependent rests on this as a premise | yes |
| `justification` | **REVIEW** — the premise moved, but the decision may still hold | yes |
| `spawn` | **REVISIT** — the dependent exists only because of this resolution | yes |
| `gate` | **UNBLOCK?** — this gate may have lifted | yes |
| `citation` | mechanical — update the reference | no |
| `evidence` | inbound support — informational | no |
| `related` | see-also — no propagation | no |

**Axis 1 — propagation semantics.** The load-bearing distinction is `premise` vs
`justification`: *obligatory* vs *advisory* propagation. A `premise` edge means the
dependent was **wrong** until the change is folded in. A `justification` edge means
the dependent was **motivated by** the source but stands on its own — the source can
change and the dependent may survive untouched. Collapsing these two loses the
difference between "this breaks" and "check whether this breaks."

**Axis 2 — recursion.** `premise / justification / spawn / gate` are **paths**: a
change ripples through them to their own dependents, transitively. `citation /
evidence / related` are **leaves**: surfaced once, not chased. A citation is a
reference to fix; a premise is a chain to walk.

---

## Why multiple types, not one

The falsifier for a single `depends_on` edge was a lived cascade. When **Q7**
resolved (`docs/questions/resolved/q7-effectful-bodies-which-monad.md`), three items
depended on it — and each required a *different* response:

- **RUNG-CT §6** was a `premise` → its framing was *wrong* until folded. Hard propagate.
- **the blocking-client decision** was a `justification` → the premise moved, but the
  decision *held* (Q7 confirmed "no architectural debt"). A single edge type that
  auto-cascaded "invalidate" would have **wrongly flagged a decision that was fine.**
- **Q8** was a `spawn` → not invalidated at all; a child to revisit.

One edge type must pick one propagation rule and be wrong for the other two.
Multiple typed edges are not ornamentation — they are the minimum expressiveness the
domain forced.

---

## The discipline: surface, never mutate

`_reach.py <id>` computes the blast radius of a change and **prints it for review. It
changes nothing.** The graph surfaces what to look at; the human judges each edge.
This is the same *produce-first / gate-second* discipline the rest of the system runs
on — a changed premise does **not** auto-invalidate its dependents, because
`justification` edges exist precisely to hold decisions that survive their premises.

```
python _reach.py q7          # what must be reviewed if q7 changed?
python _reach.py --graph     # the whole typed edge list
```

Automation stops at *surfacing*. Propagation is a review checklist, not a rewrite.

---

## Scale, and what is load-bearing

The store is inconsequential and will change: **frontmatter** now (clone-and-read, no
service), SQLite next, a real graph store eventually if a registry outgrows the
filesystem. What is load-bearing is the **model** — a graph of *typed* relationships
between items, where propagation is *typed reachability* — and it outlives whatever
holds it.

The typing is also the evidence for **Q9**. If a single edge type sufficed, the
dependency structure would be plain reachability and there would be no superstructure
to name. The fact that propagation semantics depend on edge *kind* — and that
advisory (`justification`) edges may break the composition an obligatory chain
assumes — is exactly what suggests the structure is richer than a graph (a presheaf,
a fibration, or a dependency-edge-as-optic mirroring the Q7 Prism result one level
up). Adding an edge type is cheap; but each new type should point at a *lived*
instance, never a speculative one. The seven above each have one.
