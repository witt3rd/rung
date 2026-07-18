# Open Questions — advancing the ladder language

**2026-07-18 · forward-looking, non-normative.**

The normative description of the language is [`SPEC.md`](../SPEC.md); the theory
correspondence is [`RUNG-CT.md`](RUNG-CT.md); the design history is
[`RUNG-RS.md`](RUNG-RS.md). This document records what remains *open* now that the
core linear-ladder language is feature-complete.

## Framing

The core linear-ladder language is done: sealed `!Send` tokens, `#[must_use]`,
private constructors, an auto-injected progress guard, terminal payloads,
error-path recovery, and continue arms — each backed by a conformance test. What
remains is not "features waiting to be implemented." It is questions with no
off-the-shelf answer, along two axes:

- **Deepen** — make the *contract* stronger (close the non-guarantees).
- **Grow** — make the language *say more* (new expressiveness).

Each question below is tagged **open** (ours to push), **parked** (known
mechanism, not worth the cost yet), or **blocked** (depends on something outside
this project).

---

## Axis 1 — Deepen the guarantees

### Q1 — Transition-body correctness *(open)*

**Question.** The type proves a transition *ran*; it does not prove the logic was
*right* (the resource was actually available, the policy actually held). How much
can the macro verify from the ladder declaration *alone*, and what must the user
supply?

**Why it's open.** This is the typestate → formal-verification boundary. From the
graph you get, for free, a discoverable set of structural invariants: every state
reachable, no unreachable `StepOutcome` variant, recovery always yields a valid
rung, no panic in any reachable transition sequence — all expressible as a
`proptest` harness the macro could emit. *Domain* invariants ("after `claim`, the
resource is claimed") are not in the graph; they need either user-supplied
contracts or a verification backend.

**Most promising angle.** Two experiments, increasing in ambition:
1. Macro emits a `proptest` harness that drives random valid transition sequences
   and asserts the structural invariants above. Cheap, catches regressions.
2. Extend the DSL with per-transition **pre/post contracts** and discharge them
   with a Rust verifier — **Kani** (model checking) or **Creusot**/**Prusti**
   (refinement). The research is whether a generated contract can actually be
   expressed and proven. This is the genuinely novel spike.

### Q2 — Cross-crate provenance *(parked)*

**Question.** Once a token (`Work::Active`) crosses a crate boundary, the
receiving crate trusts it. Can that be sealed — and is a lighter mechanism than a
whole sub-crate possible?

**Why it's parked.** The mechanism is already known: emit the sealed types into a
sub-crate that only the macro controls, so even the defining crate cannot
fabricate. That closes it — at the cost of one crate per ladder. Lighter ideas
(sealed traits, a zero-sized provenance capability threaded through a run) don't
actually seal, because the capability can itself be minted in-crate. So this is
engineering-with-a-cost, not deep research, and YAGNI until a real multi-crate
architecture demands it.

### Q3 — True no-drop / linearity *(blocked)*

**Question.** `#[must_use]` is escapable (`mem::forget`, `let _ = token;`, burying
the token in a dropped container). Rust is affine, not linear. Can a live token be
made truly impossible to drop?

**Why it's blocked.** This needs *linear* types — a language feature Rust does not
have and a lint cannot emulate. It is not ours to close.

**Adjacent angle.** A different substrate — a linear language (Austral, Idris,
Lean, or a future Rust with linear types) — would host the ladder faithfully.
Because the ladder is a morphism in a linear/dagger category (see the CT map
below), a linear host would make that correspondence *exact* and close no-drop for
free. The experiment: port one ladder to a linear substrate and measure what
closes.

---

## Axis 2 — Grow the expressiveness

### Q4 — Composition / nested ladders *(open)*

**Question.** Can a rung's payload be a *completed sub-ladder run*? How do you type
"this `Active` was produced by running sub-ladder S to completion," and how do
ladders compose?

**Why it's open.** This is the free-category / indexed-monad structure the theory
already names (see the CT map below). The unknowns are the surface syntax (a
ladder-of-ladders) and how carry/provenance thread across the nesting boundary.
Most tractable of the growth questions — the theory says the structure is there.

### Q5 — Fork-join / concurrency *(open)*

**Question.** Can a transition split one token into N independent linear
sub-tokens, run them concurrently, and join by consuming all of them exhaustively?

**Why it's open.** This deliberately bends the one-token contract (one → many →
one) that everything else rests on. The hard part is the *type* of a split rung:
how to yield a set of independent linear tokens and force the join to consume every
one. Genuinely unsolved, and where real workflows live.

### Q6 — Genericity *(open)*

**Question.** Can ladders be parameterized over payload/carry types (a generic
ladder instantiated per use)?

**Why it's open.** Mostly a macro-engineering question (generic parameters through
the emitted module), but the interaction with the sealed-constructor visibility
rule (G2) and the progress-guard bounds (G8) needs design.

### Q7 — Effectful transition bodies: which monad? *(open — the Kleisli framing is a conjecture, verify first)*

**Question (reframed 2026-07-18).** A transition body returning
`Result<StepOutcome, Failed>` is already an *effectful* arrow — plausibly a
**Kleisli morphism**, an arrow `A → M B` that carries an effect `M`. If that
reading is rigorous:

- the **error path** is a Kleisli arrow in the `Result` monad — *shipped, via the
  recover edges*;
- an **async** body is a Kleisli arrow in the `Future` monad — *the async case*.

Kleisli composition is one law regardless of the monad, so an async body would
compose *the same way the error path already does*. Under this framing the
question is not "how do we bolt async onto rung" but **"rung's arrows are Kleisli
arrows — which monad?"** — and Q7 shrinks from a hard new feature into an
extension of structure already owned. Its answer may be half-written in the
recover edges.

**⚠️ The framing is unverified — this is the load-bearing conjecture.** Whether
`Result`/`Future` bodies are Kleisli arrows in the *precise technical sense*, or
only a loose analogy, has not been checked against the real math. Two references
to ground it against: **Fritz** (Markov categories — note that a *generative* body,
an LLM call, is a **stochastic map / Markov kernel**, which is not obviously the
`Result` or `Future` monad; the membrane's monad may be a *probability* monad, and
Kleisli-over-a-Markov-category is where that composition is defined) and
**Capucci/Hedges** (categorical cybernetics, optics). **Verify the monad framing
before building on it.** If it holds, most of the async work is free; if it's
loose, async is a genuine new feature and the async unknowns below stand alone.

**The async unknowns (independent of the framing).**
1. **Does the guarantee survive `.await`?** A token moved into an async body is
   still moved *once* — linearity holds. `!Send` is arguably *consistent* with
   async: a `!Send` token held across an await pins the future to one thread —
   exactly the one-token-one-thread contract (G3) — but that makes the future
   itself `!Send`, so it cannot run on a multi-threaded executor. Whether that
   composes with a real driver or forces a single-threaded runtime is the
   load-bearing async question.
2. **What drives an async ladder?** Today the driver is a sync loop
   (`match step(token) { .. }`); an async transition needs an async drive, and a
   ladder mixing sync and async transitions needs both.

**Relationship to Q5.** Adjacent but orthogonal — Q5 (fork-join) splits one token
into N concurrent sub-tokens; this is a *single* transition that awaits.

**Experiments when it ripens.**
1. A per-transition `async` marker in the DSL (`async active = |spec| { .. }`),
   emitting `async fn` and an async drive; sync and async transitions coexist.
2. Verify the `!Send` / await interaction against a real executor.

**Provenance.** Surfaced 2026-07-18 choosing the client for the first
deterministic→generative fold; the blocking-client (`ureq` / `reqwest` blocking)
decision is the deliberate YAGNI park — build the smallest real thing, let it tell
us when async earns its place. Kleisli reframe from the 2026-07-18 handoff
(`2026-07-18_HANDOFF_verbs-are-arrows.md`).

---

## The category-theory map as a question generator

[`RUNG-CT.md`](RUNG-CT.md) is not decoration — its structure predicts features.
The **growth** questions (Axis 2) all come from one place: the standard
categorical **ascent**, where each level is "the arrows of the category one level
up." That tower, not an ad-hoc feature list, is the principled generator.

### The growth tower

| Level | Arrows *of* | Are | In rung |
|---|---|---|---|
| 0 | a category | **morphisms** | a transition (`Gathered => Evaluated`) — **have it** |
| 1 | **Cat** (the category of categories) | **functors** | ladder-to-ladder maps — **Q4** (nesting / composition) |
| 2 | **Fun** (the functor category) | **natural transformations** | *present, unused* — a reserved slot |

**Level 1 is already instantiated.** A registry composing per-item ladders is a
functor situation: the outer arrow (`open → resolved`) is *witnessed by* the inner
ladder reaching its terminal. That witnessing relationship is what Q4 formalizes.

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

## Triage

- **Blocked externally:** Q3 (true linearity ← the language).
- **Known but costly — park:** Q2 (cross-crate provenance ← sub-crate).
- **Open and ours to push:** Q1 (verifier spike), Q4 (composition — Level 1 of the
  tower), Q5 (fork-join), Q6 (genericity), Q7 (effectful bodies / *which monad?* —
  Kleisli framing pending verification; async parked behind the blocking-client
  choice).

**Verify before building.** One item gates real work: the **Kleisli framing** (Q7)
is a conjecture, not a result — check it against Fritz / Capucci before any async
or effect work leans on it (2026-07-18 handoff).

**Highest-information next experiments.**
- To *deepen*: the Q1 Kani/Creusot spike on a single ladder — does a generated
  per-transition contract prove?
- To *grow*: Q4 composition — the free-category structure the theory already
  guarantees is present, and the smallest real step beyond a linear ladder.
