# INTAKE — how a question enters the registry

**Date:** 2026-07-19 · **Status:** v0 — "maybe this is right?" A first pass at making an *implicit habit* explicit, expected to be revised through use. Companion to the [README](README.md) (the *back half* of a question's life) and [`../EDGES.md`](../EDGES.md) (the edge vocabulary). If a step here proves wrong in practice, fix it — this doc is itself a question that resolves through iteration.

---

## Why this exists

The README specifies the *back half* of a question's life richly — folders, the three laws, resolved-vs-owed, typed edges. But the *front half* — how a raw question becomes a tracked file — lived only as an undocumented habit: someone notices one mid-conversation and hand-writes a file. That works at low volume and in hot context, and **fails silently the moment a question surfaces when no one stops to catch it.** The registry exists to defeat tears-in-rain — *the bet placed and never collected* — but a bet only gets **placed** if intake happens. This is the capture door, written down so it stays open consistently.

Lived proof it was needed: Q9 was caught only because a hot-context reader remembered the Q7 cascade mid-session. Three weeks cold, it would have evaporated before it was ever a file. That is the exact gap this document closes — one level up from the cascade gap the registry already defends.

## What counts as a question worth filing

Not every uncertainty. File it when **all three** hold:

1. **It has a resolution condition.** You can state what would count as an answer — even roughly. *"Which monad do effectful bodies live in?"* qualifies (Q7 — answerable against the categorical literature); *"I wonder about effects"* does not.
2. **Something rests on it, or will.** It blocks, gates, or grounds real downstream work. Q7 gated the blocking-client decision; Q9 grounds RUNG-CT's Level 1. If nothing waits on it, it's a musing, not a tracked bet.
3. **It would otherwise evaporate.** If it's already captured somewhere load-bearing — a `SPEC.md` guarantee, a `RUNG-CT.md` section — don't duplicate it. The registry is for the ones that would be lost.

If it doesn't clear all three, it's not rejected — it's just not a *registry* item yet. A one-line note elsewhere is fine.

## The two doors — captured *and* generated

A question reaches intake by one of two paths, and **both go through the same procedure below.** Naming the split is what keeps rung's registry honest about where its frontier comes from:

- **Captured** — a question surfaces in lived work: a design fork, a review, a cascade. Q7 arose this way (the blocking-client decision needed to know whether async was "half-written in the recover edges").
- **Generated** — [`_map.md`](_map.md), the category-theory growth tower, *predicts the next question* from the structure itself. Q4 fell out of the functor level ("ladder-to-ladder maps"); Q9 fell out of "what overlays the ladder." The generator is a **source that feeds intake, not intake itself** — the tower proposes, a human still reads it and files the file. There is no auto-filing; the generative door still passes through the same capture procedure and the same three-part test.

This is what rung has that a capture-only registry does not: a principled generator for the front half. Use it — when you resolve a question, ask `_map.md` what the resolution *predicts* next, and file that too if it clears the bar.

## The procedure

1. **Name it as a question** — one sentence, as sharp as you can make it now. Sharpness can improve later; vagueness at intake is allowed, but the *shape* must be a question, not a topic.
2. **Assign the next Q-number.** rung's IDs are stable `qN` anchors, cited from `RUNG-CT.md`, `SPEC.md`, and other question files. Take the next free integer; **never renumber an existing one.**
3. **Pick the folder** (folder is status):
   - `open/` — unresolved, actively ours to push.
   - `blocked/` — waiting on something outside this project (a language feature, an upstream dependency). Name what with a `gate` edge.
   - `parked/` — a known answer-path exists but isn't worth the cost yet (YAGNI). *(Create the folder on first use.)*
   - `resolved/` — reached later, never at intake. A question is not born answered.
4. **Write the body** — the parts the README names and the resolved files model (see `resolved/q7-*.md` as the template): the question; provenance (who/when/where it arose); *what would count as an answer*; what it blocks; a `## State` log with the opening line dated.
5. **Add the frontmatter** — machine-readable, at the very top:
   ```yaml
   ---
   id: q<N>
   status: open
   depends_on:                      # what THIS rests on — LIVED edges only, omit if none
     - {on: <other-id>, kind: premise}
   affects:                         # what a change to THIS ripples to — omit if none
     - {target: <doc-or-decision>, kind: premise}
   ---
   ```
   - **Only encode a lived edge** — a dependency that actually exists *right now*. Never a speculative one. If you're unsure whether an edge is real, leave it out: a missing edge is cheap to add when it becomes real; a phantom edge is noise that erodes trust in the graph. (Edge kinds and their propagation semantics: [`../EDGES.md`](../EDGES.md).)
6. **Run the check** — `python _reach.py --graph` to confirm the file parses and any edge shows up. If it doesn't appear, the frontmatter is malformed.
7. **Keep both status markers.** The prose `**Status:**` line (human) and the frontmatter `status:` (machine) are redundant on purpose; set both to the same value.

## What intake is NOT (the boundary, held on purpose)

**Intake is capture, not answering.** This procedure files a question you already have (or the generator just surfaced). It does not resolve it — that is the README's back half, gated by the three laws (an answer must land in a normative surface before the file moves to `resolved/`).

And the generative door does **not** collapse into auto-generation. `_map.md` predicts *candidates*; a human judges each against the three-part test and files it deliberately. A registry that auto-fills itself from the tower would drown in speculative questions — the exact noise the lived-edge discipline exists to prevent, one level up.

## The honest edge

This is v0. It documents the capture habit and names the two doors; it does not yet address **batch intake** (many questions at once, after a big session), **de-duplication** (is this already filed under a different Q-number?), or hardening the generative door into anything more than "read the tower and judge." Those are the next iterations — taken when a lived instance forces each, never before. *Good, now; toward perfect, through use.*

---

*Drafted by Forge ⚒️, 2026-07-19, at Donald's direction — completing the pair with Augur's `genesis/meta/questions/INTAKE.md` (drafted the same morning). The two registries cross-reference: augur points here for the edge vocabulary; rung has the `_map.md` generator augur named as its own missing piece. Festina lente.*
