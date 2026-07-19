# docs/questions/ — The Open-Question Registry for rung

*A question is a bet that some resolution will hold. This is where each one lives until it does.*

Every open question about advancing the ladder language gets its own document here, so it can **receive an answer over time** rather than living as a bullet buried in a design doc where it is easy to lose and impossible to track. When a question resolves, its answer **folds upward** into the normative surfaces — `SPEC.md` (the guarantees), `RUNG-CT.md` (the theory), the macro itself — and the question document records *that it happened, and where.*

This is the `outer-loop/bets/` and `augur/genesis/meta/questions/` pattern, applied to rung itself. The same laws that keep those registries from decaying into a bitbucket apply here. rung is a language for declaring the objects and legal arrows of a category; this registry is the same discipline pointed at rung's own frontier — and, fittingly, its own lifecycle (`open → resolved`) is itself a ladder.

---

## Folder is status

```
docs/questions/
  open/       — unresolved. actively awaiting an answer, and ours to push.
  parked/     — a known mechanism exists, but the cost isn't worth paying yet (YAGNI).
  blocked/    — depends on something outside this project (e.g. a language feature).
  resolved/   — the fold is complete. The answer has landed in a normative surface
                (SPEC / RUNG-CT / the macro) AND every owed change there is done.
                Not "answer found" — "answer propagated." If any fold is still owed,
                the file stays out of resolved/.
```

Moving a file between folders is the lifecycle. Git history records every transition for free. Each question keeps its stable **Q-number** as an ID (referenced from `RUNG-CT.md`, `SPEC.md`, and the handoff) — the number is the anchor; the folder is the status.

## The three laws

1. **A question is not resolved until its answer has changed something normative.** Moving a file to `resolved/` requires that the resolution actually landed in `SPEC.md`, `RUNG-CT.md`, or the macro — wherever it belongs. The resolution document records *what changed and where*. A question doc that says "answered" but points nowhere is not resolved; it is just a doc that stopped being maintained.

2. **This registry tracks; the normative surfaces receive.** The question documents are the audit trail — what was asked, when, and how it resolved. The *answer itself* lives in the guarantee, the spec section, or the code it changed, cited back to the question. Do not let a question document become a second home for a rule the spec owns; that is how you get two sources of truth that drift.

3. **To be tracked is to live in a folder that gets scanned; a note inside a done-folder is documentation, not tracking.** Status is *where the machinery looks*, not what the prose says. `open/` and `parked/` and `blocked/` get scanned for owed or waiting work; `resolved/` (correctly) does not — that is what makes it a done-pile. So a "still owed" note living inside a `resolved/` file is documented but *not tracked*: it sits in the one place designed never to be looked back at. Any remaining action keeps its file out of `resolved/`, full stop. (This law was learned the hard way in the sibling registry — a question mis-filed to `resolved/` with a fold still owed, caught only from outside. The fix is never "scan the done-pile"; the fix is that a file with owed work is in the wrong folder.)

---

## The frontier at a glance

| Q | Question | Status |
|---|---|---|
| Q1 | Transition-body correctness (can the macro verify the logic, not just that it ran?) | **open** |
| Q2 | Cross-crate provenance (seal a token across a crate boundary) | **parked** |
| Q3 | True no-drop / linearity (`#[must_use]` is escapable; Rust is affine, not linear) | **blocked** |
| Q4 | Composition / nested ladders (a rung's payload is a completed sub-ladder run) | **open** |
| Q5 | Fork-join / concurrency (split one token into N, run concurrently, join exhaustively) | **open** |
| Q6 | Genericity (ladders parameterized over payload/carry types) | **open** |
| Q7 | Effectful transition bodies: which monad? | **resolved** — transitions are **Prisms**, not Kleisli arrows |
| Q8 | The async driver (a free-standing feature, per Q7's resolution) | **open** |
| Q9 | The dependency superstructure — what overlays the ladder level? | **resolved** — a **Grothendieck opfibration** of dependent optics |

**The growth tower** (`morphisms → functors in Cat → natural transformations in Fun`) and the CT-map-as-question-generator live in `_map.md` — the category theory is the principled source of the growth questions, not an ad-hoc list.

**Verify before building** was the standing gate on Q7. It has now been walked: two independent expert reviews resolved it (see `resolved/q7-effectful-bodies-which-monad.md`). The lesson generalizes — a sharp, falsifiable question, checked against the real literature, is exactly what this registry is for.

---

## Dependencies — the registry is a typed graph

Questions are not independent. When Q7 resolved, three things rested on it and had to be re-examined: RUNG-CT §6 (which used its framing), the blocking-client decision (justified by it being unresolved), and Q8 (spawned by the resolution). Today those edges live only in prose — legible to a human re-reading every file, invisible to any traversal. That is a **tears-in-rain** gap one level up: the cascade is caught only if a hot-context reader happens to remember it.

The full vocabulary and its rationale are in [`../EDGES.md`](../EDGES.md); the quickstart follows. Each file carries **typed dependency edges** in frontmatter:

```yaml
---
id: q7
status: resolved
depends_on:                              # what THIS rests on
  - {on: kleisli-reviews, kind: evidence}
affects:                                 # what a change to this ripples to
  - {target: RUNG-CT§6, kind: premise}
  - {target: blocking-client-decision, kind: justification}
---
```

The **kind is load-bearing** — it decides how a change propagates, and no single rule fits all:

| kind | when the source changes… |
|---|---|
| `premise` | **MUST re-examine** — the dependent rests on this as a premise |
| `justification` | **REVIEW** — the premise moved, but the decision may still hold |
| `spawn` | **REVISIT** — exists only because of this resolution |
| `gate` | **UNBLOCK?** — this gate may have lifted |
| `citation` | mechanical — update the reference |
| `evidence` | inbound support — informational |

`_reach.py` walks it and prints the blast radius **for review — it never mutates.** The graph surfaces what to look at; the human judges each edge. (A changed premise does not auto-invalidate its dependents — same produce-first / gate-second discipline as the rest of the system.)

```
python _reach.py q7          # what must be reviewed if q7 changed?
python _reach.py --graph     # the whole typed edge list
```

The implementation is deliberately minimal — frontmatter + a stdlib script, preserving "clone and read, no service to run." It is honest at this scale (SQLite next, a real graph store eventually, if the registry ever outgrows the filesystem). **The model is what matters, not the store:** the registry is a *graph of typed relationships between items*, and propagation is *typed reachability*. That model outlives whatever holds it.

> **This is a Level-1 structure.** The growth tower in `_map.md` names it: Level 0 is arrows *within* a category (a transition); Level 1 is arrows in **Cat** (functors — maps between whole structures). A dependency is an arrow between *items*, not within one — the registry itself is a Level-1 object. The tower predicted it. **What that superstructure *is* precisely is now resolved (Q9): a Grothendieck opfibration whose fibres are the per-item ladders and whose typed edges are dependent optics** — the Q7 Prism result, one level up. See `resolved/q9-the-dependency-superstructure.md` (folded into `../RUNG-CT.md` §10 and `../EDGES.md`); `_reach.py` computes its deflationary boolean shadow.
