---
id: q9
status: resolved
depends_on:
  - {on: _map:growth-tower, kind: premise}
  - {on: q7, kind: premise}
  - {on: q9-reviews, kind: evidence}
affects:
  - {target: RUNG-CT, kind: premise}
  - {target: EDGES, kind: premise}
---

# Q9 — The dependency superstructure: what overlays the ladder level?

**Status:** RESOLVED (2026-07-18, two independent CT reviews converge) · **Verdict:** the dependency superstructure is a **Grothendieck opfibration** over the free category on the typed edge graph, and its typed edges are **dependent optics** — the Q7 Prism result, mirrored one level up. `_reach.py` computes the deflationary boolean *shadow* of it.

## The question

When Q7 resolved, it did **not** change one thing. It changed a *reachable set* along **typed** edges — RUNG-CT §6 (premise: wrong until folded), the blocking-client decision (justification: held despite the premise moving), and it spawned Q8. The propagation *rule depended on the edge type*. We built the operative stopgap (typed frontmatter + `_reach.py`) the same session, but the stopgap computes boolean reachability — and Donald's read was that this is *bigger than "the action is in the arrows,"* a whole layer of the solution space we might be missing. The sharp question, tee'd for external review:

> Given items with states and *typed* directed edges (`premise / justification / spawn / gate / …`), each type carrying a distinct propagation semantics: is this (a) transitive closure of a typed relation, (b) a presheaf, (c) a fibration whose cartesian lifts are the typed propagations, or (d) something else? Does the obligatory-vs-advisory distinction force structure richer than a graph — and does advisory propagation break functoriality? Is a dependency edge itself a (dependent) **optic**, mirroring the Q7 result one level up?

## What counted as an answer

Two independent outside reviews, checked against the categorical literature (Grothendieck fibrations, Moggi/Fritz on monad strength and Markov categories, Capucci–Hedges on optics, Spivak's *Poly* / Hedges' *Open Games* for the cybernetics frame), converging on a structure — the same bar that closed Q7. Full reviews: `../resolved/_evidence/q9-review-1-fibration.md` and `../resolved/_evidence/q9-review-2-opfibration.md`.

## Resolution

**Both reviews converge; they diverge on one point that resolves cleanly in the sharper review's favor.**

**1. The superstructure is a Grothendieck opfibration.** Candidate (a) transitive closure is the *skeleton* we already have — it computes a blast-radius set but erases *state kinematics*: it cannot see that after an advisory intermediate is reviewed, its state may be unchanged, extinguishing further propagation. Candidate (b) presheaf models the system *at rest* (static consistency, a sheaf condition on "a resolution is consistent across its dependents") but does not supply the *lifts* that turn a premise-change into an obligation. The primary model is a **fibration** `p : E → B`:

- **Base `B`:** the free category on the typed edge graph — objects are items, generating morphisms are the typed edges.
- **Fibres `E_x`:** the possible states of item `x` — *exactly its Level-0 ladder.*
- **Total space `E`:** items-in-specific-states.
- **Typed edges are the lifts:** a `premise` edge is a strict lift that *forces* re-examination; a `justification` edge is a weaker/optional lift; a `citation` edge is a mechanical state-update.

**The orientation is an opfibration, not a fibration.** Review 1 named the genus (*fibration*) and described forward transport without committing the co-/contra- direction; Review 2 committed it, and it has the orientation right. Our edges point *along information flow* — `X → Y` means Y depends on X, so a state-change at X **pushes forward** to Y. Forward transport is **opcartesian** (a pushforward functor `f_! : E_X → E_Y`), not a cartesian pullback. This is a sharpening, not a conflict.

**2. "Advisory" does not break functoriality — it maps into the Q7 coproduct.** The dangerous falsifier: an obligatory edge deterministically maps `X → Y`, but an advisory edge relies on a human "no change needed," which appears to violate strict functoriality (a rigid functor cannot map X's recovery into a state Y is already in). The recovery is the structure **Q7 already gave us.** An advisory pushforward does not land in Y's base category of rungs; it lands in the **Kleisli category** of Y, delivering a **coproduct** `ReviewRequired(Y) + Survives(Y)`. The Level-1 superstructure functorially delivers that coproduct; Y's own Level-0 machinery (its effectful transition body, its progress guard) then collapses the uncertainty — and because the fibres are *free categories* (§1), Y retains agency to run vertical morphisms (a manual `Review → Resolved`) entirely within its own fibre. **Q7's coproduct is exactly what absorbs Q9's advisory shockwave.** The two levels are load-bearing on each other. *(This is why Q9 depends on Q7 as a premise: if Q7's Kleisli/coproduct result moved, this resolution must be re-examined.)*

**3. The tower is self-similar — edges are dependent optics.** Q7 established that a Level-0 transition is a **Prism** (an optic for sum types: forward Match + backward Build). A Level-1 dependency is a **Lens** (an optic for product/context: bidirectional transport across a boundary), and because the backward-pass type (the blast radius) depends on the exact state transported forward, it is a **dependent optic**:

- **Forward pass (play):** covariant — the opcartesian pushforward of a state-change down the graph to dependents.
- **Backward pass (coplay):** contravariant — before modifying X, query backward along the optic; each Y computes its exposure *from its current state* and returns a typed cost. **Blast-radius is the backward pass, not a file count** — an exposure vector (*"3 mechanical updates, cheap; 2 obligatory coproduct reviews, expensive"*), the support of the composite optic.

The growth tower predicted a Level-1 structure; Q9 names it precisely. Level 0 transitions are Prisms; Level 1 dependencies are Lenses; the whole is an opfibration over a category of dependent optics.

## What folded upward, and where

- **RUNG-CT gains §10 — "The Level-1 superstructure."** The tower's Level ≥1 was a *prediction*; it is now a *described structure*: a Grothendieck opfibration whose fibres are the Level-0 ladders and whose typed edges are dependent optics, with advisory functoriality recovered via the Q7 coproduct. *(Fold target: `RUNG-CT.md` §10; Summary renumbered §11.)*
- **EDGES.md's closing is now resolved, not speculative.** It framed the superstructure as an open guess ("a presheaf, a fibration, or a dependency-edge-as-optic"); that speculation is closed. The `_reach.py` growth-path it describes is now principled: *stop computing boolean reachability; compute transport of typed obligations* — the support of the composite dependent optic. The store stays frontmatter for now; the model is named. *(Fold target: `../EDGES.md`. The `affects` edge that motivates the registry-growth consequence is retyped from `RUNG-RS` — the reviewers' shorthand for "the registry side" — to **EDGES**, its true home in this tree. RUNG-RS.md is the Rust *language* spec and is genuinely untouched by the registry meta-structure.)*
- **`_map.md` — the growth tower gets a second named Level-1 instance.** Level 1 was "functors in Cat — Q4 (nesting)"; the dependency opfibration is a *distinct* Level-1 functor (`p : E → B`, ladders as fibres), so the generator now names both, so it does not contradict its own resolved question.
- **The Kleisli tie is now bidirectional.** Q7 said error and effects are orthogonal gadgets *within* a transition; Q9 shows the same coproduct is what keeps the *dependency* level functorial. The self-similarity is not analogy — it is the same categorical machinery loading two levels of the tower.

## State
- 2026-07-18 (Q7 cascade) — Donald surfaces the phenomenon: *"a dozen things needed to update to ripple the consequences… a super-structure that overlays the current ladder level, worth deeply understanding."* Registry stopgap (`_reach.py`) built same session; Q9 tee'd for review.
- 2026-07-18 — two independent CT reviews commissioned and returned.
- 2026-07-18 — both converge: **opfibration + dependent optics.** Divergence (fibration vs opfibration; hedged vs committed optic) resolves toward the sharper review. Folded into RUNG-CT §10, EDGES.md, `_map.md`; this file moved to `resolved/` because the fold is real and complete — no owed change left as a note inside a resolved file (law #3).
