---
id: q9
status: open
depends_on:
  - {on: _map:growth-tower, kind: premise}
affects:
  - {target: RUNG-CT, kind: premise}
  - {target: RUNG-RS, kind: premise}
---

# Q9 — The dependency superstructure: what overlays the ladder level?

**Status:** OPEN · **class:** foundational / theory-first · **for:** external CT review (tee'd up 2026-07-18)

> **This is not a feature request. It is a "we may be missing a whole layer of the
> solution space" question** — Donald's read is that this is *bigger than the
> recognition that the action is in the arrows*, and worth understanding before we
> build much more. The registry mechanism (typed frontmatter edges + `_reach.py`)
> is the *operative stopgap*; this question is about the *structure that stopgap is
> approximating*.

## The phenomenon (lived, not hypothesized)

When Q7 resolved (the Kleisli conjecture — see `resolved/q7-effectful-bodies-which-monad.md`),
it did **not** change one thing. It changed a *reachable set*: RUNG-CT §6's framing
(premise), the blocking-client decision (justification — which *held* despite the
premise moving), and it spawned Q8. A single node's state change **propagated along
typed edges**, and the propagation *rule depended on the edge type* — some edges say
"must re-examine," others say "review, may still hold," others "mechanical update."

We had **no mechanism** for this. The edges existed only in prose. The cascade was
caught only because a hot-context reader (Forge, mid-session) remembered it. Three
weeks cold, it would have been tears in the rain. This is the *same class of gap* the
registry itself exists to prevent (a bet resolving without its consequences folding
upward) — but **one level up**: not "a question loses its answer," but "a resolution's
consequences fail to ripple."

## Why this is a superstructure, not just a bigger ladder

The ladder (rung Level 0) is arrows *within* one category — a transition `A → B`. The
growth tower (`_map.md`) already names the ascent:

- **Level 0** — morphisms (a transition). *Have it.*
- **Level 1** — functors, arrows in **Cat** (ladder-to-ladder). *Q4.*
- **Level 2** — natural transformations, arrows in **Fun**. *Reserved, unused.*

A **dependency is an arrow between whole items** — between questions, between claims,
between a resolution and the sections that rested on it. That is not a transition
*inside* a ladder; it is structure *over* the collection of ladders/claims. It lives
at Level 1 or above. **The tower predicted that a superstructure exists here** — this
question asks *what it is, precisely*, and what it buys us to name it correctly.

## The candidate formalisms (name honestly, do not pre-commit)

Each is a genuine hypothesis with a different payoff. We do **not** know which holds;
that is the question. (Same discipline as Q7: name the falsifiers, let the experts
resolve.)

1. **Transitive closure of a typed relation.** The deflationary answer: it's just a
   directed graph with edge-types, and "blast radius" is reachability. `_reach.py`
   implements exactly this. *Payoff if true:* nothing deeper to find; the stopgap is
   the answer, and we harden it (SQLite, etc.) and move on. *Falsifier:* if the
   propagation rule genuinely depends on *paths* (composition of edge-types) rather
   than just reachability, a flat typed relation under-describes it — see below.

2. **A presheaf** — a functor `Cᵒᵖ → Set` assigning to each item the set of things
   that depend on it, contravariantly. The dependency structure as *"what varies over
   the base category of items."* *Payoff if true:* the machinery of sheaves/topoi
   (gluing, locality, restriction) becomes available — "a change is local until it
   isn't" gets a precise meaning, and *consistency* of a resolution across its
   dependents becomes a sheaf condition. *Question:* is there a Grothendieck topology
   here — do the edge-types define covers?

3. **A fibration** — a functor `E → B` where the "total space" `E` is
   claims-with-their-dependencies and `B` is the base of items, with cartesian lifts
   modeling how a change in the base *transports* along dependency edges. *Payoff if
   true:* propagation is *transport in a fibration* — the edge-types are the
   structure that says how a base-change lifts, and "does this decision still hold
   when its premise moved?" is a cartesian-lift question. This is the richest
   hypothesis and the most likely to explain the *typed* propagation.

4. **Effectful / typed edges as their own Prism-analogue.** Tonight's Q7 result was
   "transitions are Prisms — forward pass + typed backward pass." A dependency edge
   *also* has two directions: forward (a resolution propagates consequences) and
   backward (before building, measure exposure = `count(reachable)`). **Is a
   dependency edge itself an optic?** — forward = propagate, backward = audit blast
   radius, on one structure. If so, the same categorical gadget describes both the
   transition level and the dependency level, and the tower is self-similar in a
   precise sense.

## The sharp question (sendable as-is to a category theorist)

> Consider a finite set of *items* (questions, claims, decisions), each with a state,
> and a set of *typed directed edges* `premise / justification / spawn / gate /
> citation` between them, where each type carries a distinct *propagation semantics*
> (must-re-examine / review / revisit / unblock / mechanical) for how a state-change
> at the source obliges action at the target.
>
> 1. Is this best modeled as (a) the transitive closure of a typed relation, (b) a
>    **presheaf** on a base category of items, (c) a **fibration** whose cartesian
>    lifts are the typed propagations, or (d) something else?
> 2. Does the *typing* of edges — especially the distinction between "premise"
>    (propagation is obligatory) and "justification" (propagation is *advisory* — the
>    dependent may survive the premise's change) — force a structure richer than a
>    graph? Specifically: does advisory propagation break functoriality (composition
>    of edges), and if so what recovers it?
> 3. Is a dependency edge itself a (dependent) **optic** — forward = propagate
>    consequences, backward = measure exposure before building — mirroring the Q7
>    result at the level *above* transitions?

## What resolving this buys us

- **RUNG-CT gains a section** (first-order fold target): what the tower's Level ≥1
  *is*, concretely, once the dependency structure is named. Tonight the tower was a
  prediction; this makes it a described structure.
- **RUNG-RS gains a design consequence** (second-order): if the answer is a fibration
  or presheaf, the *registry mechanism* is not a stopgap to harden but a *shadow* of a
  richer structure — the frontmatter edges should carry whatever the real structure
  demands (composition data? covers?), and `_reach.py` should compute the real
  operation (transport / gluing), not just reachability.
- **The blast-radius-as-a-number** goal (measure exposure before building on shaky
  ground) gets a principled definition instead of "count reachable files."

## Provenance

Surfaced 2026-07-18, immediately after the Q7 cascade, by Donald: *"a dozen things
needed to update to ripple the consequences… this isn't search-and-replace… a
super-structure that overlays the current ladder level, and this is worth deeply
understanding."* The registry dependency mechanism (typed edges + `_reach.py`) was
built the same session as the operative stopgap; this question is the theory it
approximates, teed up for external review.
