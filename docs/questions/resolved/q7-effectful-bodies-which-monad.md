# Q7 — Effectful transition bodies: which monad?

**Status:** RESOLVED (2026-07-18) · **Verdict:** transitions are **Prisms (optics)**, not Kleisli arrows. The error/async unification is false; recovery is the Prism's backward pass; async is a genuine free-standing feature.

## The question

A transition body returning `Result<StepOutcome, Failed>` looked like an *effectful* arrow — plausibly a **Kleisli morphism** `A → M B`. The conjecture (from the verbs-are-arrows handoff): if that reading is rigorous, then the error path is Kleisli-in-`Result`, async is Kleisli-in-`Future`, both compose by one Kleisli law, and **async is "nearly free" — its answer half-written in the recover edges.**

The handoff flagged this as the **one load-bearing conjecture: verify against the real math before building on it.**

## What counted as an answer

A check against the categorical literature (Moggi on monad strength, Beck on distributive laws, Fritz on strong/Markov categories, Capucci–Hedges on optics) resolving three outcomes:
- **(a)** strong monad + distributive laws hold → Kleisli framing rigorous, async ≈ free;
- **(b)** error/recover is the **dagger** (RUNG-CT §6), not a monad → the unification is false;
- **(c)** the transition is an **optic** → reframe from "which monad" to "an optic with a monadic effect layered on."

The cheapest falsifier: `Failed<A>` returns the *input* token, which is **not** the exception monad's shape (a fixed error `E`). If it's the §6 dagger and not a Kleisli monad, (b) holds without touching the harder questions.

## Resolution

**Two independent outside expert reviews converge. The falsifier triggers: outcome (c) holds, absorbing (b).** Full reviews: `../resolved/_evidence/kleisli-review-1-dagger.md` and `../resolved/_evidence/kleisli-review-2-prism.md`.

**1. `Failed<A>` is not a monad (the falsifier).** A Kleisli arrow in an exception monad needs a uniform endofunctor `T(X) = X + E` with `E` a *fixed* error object independent of the input. rung's transition is `f : A → B + A` — failure returns the *input* token, so `T_A(X) = X + A` depends on the domain. Trace composition `g ∘ f` with `f : A → B + A` and `g : B → C + B`: the composite has domain `A` and must return `A` on failure, but if `g` fails it hands back `B`. Standard monadic bind has **no mechanism to route `B → A`**. Because sequential composition requires explicit reverse edges to typecheck, **it is not a monad** — so Beck distributive laws do not apply, and the unification fails.

**2. Transitions are Prisms (Capucci–Hedges).** A forward pass that consumes a resource to yield a successor-or-residual, plus a backward pass routing downstream failures to the start, is the definition of an **optic** — specifically a **Prism**, because the branching is by coproduct (verdicts), not product:
- **Match (forward):** `A → Σᵢ Bᵢ + A` — *exactly the transition signature.*
- **Build (backward):** the residual `A` passes through; the successors `Bᵢ` route back via the **§6 dagger** `Bᵢ → A`. **The recover edges are the Prism's backward pass.**

**3. Async and generative are effectful optics (Fritz).** No distributive law is needed to combine effects with error. Layer a strong monad `M` on the *forward pass* of the Prism: `A → M(Σᵢ Bᵢ + A)`.
- **Async (`Future`)** is a strong commutative monad — tensorial strength `A ⊗ T(B) → T(A ⊗ B)` is exactly the property proving **linearity survives `.await`**.
- **Generative (`Prob`)** is a probability monad (Giry) in a Markov category — inherently *affine* (normalizes to 1, discarding is legal). This is why Fritz is the referee for a generative body: it's a **Markov kernel**, not `Result` or `Future`.

## What folded upward, and where

- **The Kleisli conjecture is closed — resolved NO on the unification.** "Q7's answer is half-written in the recover edges" resolves to **No**: error and async are orthogonal categorical gadgets.
- **RUNG-CT §6 (the dagger) is vindicated.** The §6-dagger intuition was exactly correct — recovery *is* the Prism's backward pass, and G8's well-founded progress guard is precisely what makes that backward pass **contractive** rather than a symmetric involution. *(Fold target: RUNG-CT.md §6 gains the Prism framing.)*
- **The blocking-client decision (cut #2) is safe.** Proceeding with `reqwest` blocking incurs **no architectural debt** — async is a new layer, not a missing piece of the error structure.
- **Q7 spawns Q8** — the async driver. Async is a genuine free-standing feature: a strong monad evaluated *before* the optic routes the token, needing a distinct async driver, but **not** a redesign of the recovery algebra. Tracked as Q8 (`../open/q8-async-driver.md`).

## State
- 2026-07-18 (handoff) — conjecture filed with the precise verification question and a "verify before building" gate.
- 2026-07-18 ~11:4x — Donald commissioned two outside expert reviews (`resolved/_evidence/kleisli-review-1-dagger.md`, `-2-prism.md`).
- 2026-07-18 — both reviews converge: **Prism, not Kleisli.** Falsifier triggered, gate resolved negative-for-unification, dagger vindicated. This file moved to `resolved/` because the fold is real: the framing is settled, the client decision is unblocked, and the residual (async driver) is tracked as its own open question — not left as an owed note inside a resolved file (law #3).
