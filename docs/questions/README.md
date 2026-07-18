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

**The growth tower** (`morphisms → functors in Cat → natural transformations in Fun`) and the CT-map-as-question-generator live in `_map.md` — the category theory is the principled source of the growth questions, not an ad-hoc list.

**Verify before building** was the standing gate on Q7. It has now been walked: two independent expert reviews resolved it (see `resolved/q7-effectful-bodies-which-monad.md`). The lesson generalizes — a sharp, falsifiable question, checked against the real literature, is exactly what this registry is for.
