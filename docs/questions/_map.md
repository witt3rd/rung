# The category-theory map — the question generator

*This is where the growth questions come from — not an ad-hoc feature list, but the
principled categorical ascent. Referenced from the registry README.*

## The category-theory map as a question generator

[`RUNG-CT.md`](RUNG-CT.md) is not decoration — its structure predicts features.
The **growth** questions (Axis 2) all come from one place: the standard
categorical **ascent**, where each level is "the arrows of the category one level
up." That tower, not an ad-hoc feature list, is the principled generator.

### The growth tower

| Level | Arrows *of* | Are | In rung |
|---|---|---|---|
| 0 | a category | **morphisms** | a transition (`Gathered => Evaluated`) — **have it** |
| 1 | **Cat** (the category of categories) | **functors** | ladder-to-ladder maps — **Q4** (nesting / composition); *and* the **dependency opfibration** `p: E → B` (ladders as fibres over the item graph) — **Q9**, resolved |
| 2 | **Fun** (the functor category) | **natural transformations** | *present, unused* — a reserved slot |

**Level 1 is already instantiated — twice.** (i) A registry composing per-item ladders is a
functor situation: the outer arrow (`open → resolved`) is *witnessed by* the inner
ladder reaching its terminal. That witnessing relationship is what Q4 formalizes.
(ii) The *dependency* structure between whole items is a second, distinct Level-1 object —
a **Grothendieck opfibration** whose fibres are the Level-0 ladders and whose typed edges
are dependent optics. That is **Q9** (resolved 2026-07-18; folded into `RUNG-CT.md` §10 and
`EDGES.md`). Two different arrows-in-**Cat**, both genuinely Level 1.

**Level 2, hold lightly.** "The same transformation applied uniformly across every
state" is where a natural transformation would live — real structure with no
earned use in rung yet. **Do not invent a use to fill it;** wait for a ladder to
need it (third-instance rule). This is the honest `…` at the top of the tower.

### The non-ascent structures (feed other questions)

| CT structure | Predicts | Question |
|---|---|---|
| Kleisli — *which monad?* | Effectful transition bodies | **Q7** — `Result` shipped (recover), `Future` async open. *Kleisli rigor is a conjecture; see Q7.* |
| Dagger | Reversibility | Recovery is a *partial, well-founded* dagger (RUNG-CT §6) — the progress guard breaks involution |
| Linear logic | A linear substrate | **Q3** |

---
