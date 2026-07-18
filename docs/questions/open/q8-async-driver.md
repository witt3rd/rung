# Q8 — The async driver

**Status:** OPEN · **Spawned by:** Q7's resolution (2026-07-18). Async is a genuine free-standing feature, not a missing piece of the error structure.

## The question

Q7 established the shape: an async transition body is a strong monad `M = Future` layered on the forward pass of the Prism — `A → Future(Σᵢ Bᵢ + A)`. The monad must be *evaluated* (awaited) before the optic can route the token. rung today has only a **sync driver** (`match step(token) { .. }`). So:

**What drives an async ladder — and can sync and async transitions coexist in one ladder?**

## Why it's open

The algebra is settled (Q7); the *mechanism* is not. Two coupled unknowns:

1. **Does the guarantee survive `.await`?** A token moved into an async body is still moved *once* — linearity holds, and Fritz's tensorial strength (`A ⊗ T(B) → T(A ⊗ B)`) is the categorical proof of it. But `!Send` interacts sharply with async: a `!Send` token held across an await point pins the future to one thread — which is *consistent* with the one-token-one-thread contract (G3), but makes the **future itself `!Send`**, so it cannot run on a multi-threaded executor. Whether that composes with a real async runtime, or forces a single-threaded executor, is the load-bearing question.

2. **Sync/async coexistence.** A ladder may mix sync transitions (deterministic bodies) and async ones (an LLM call, an inference round-trip). The driver must handle both — the sync `step` loop and an async drive — without forcing every transition async.

## Most promising angle

1. A per-transition `async` marker in the DSL:
   ```
   impl {
       async evaluated = |gathered| { /* .await an LLM judge */ },
       synthesized = |evaluated| { /* sync */ },
   }
   ```
   emitting `async fn` for marked transitions and an async drive; sync and async transitions coexist in one ladder.
2. Verify the `!Send` / await interaction against a real executor — does the pinned-future consequence force a single-threaded runtime, or does it compose? This is the experiment that closes unknown (1).

## Relationship to other questions

- **Q5 (fork-join)** — adjacent but orthogonal. Q5 splits one token into N concurrent sub-tokens; Q8 is a *single* transition that awaits. Async-single-transition and concurrent-fork-join are different concurrency shapes.
- **Q7 (resolved)** — Q8 is Q7's residual. The framing (strong monad on the optic's forward pass) is Q7's gift; the driver mechanism is Q8's work.

## Provenance

The blocking-client (`reqwest` blocking) decision for the first LLM fold (cut #2) is the deliberate YAGNI park — a batch resolver has no latency pressure, so a sync body over a blocking call is honest and correct now. Q8 is what "when async earns its place" points at. Per Q7, proceeding on the blocking client incurs **no architectural debt**: async is a new layer added later, not a redesign.
