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

### Q7 — Async transition bodies *(open)*

**Question.** A transition body is a synchronous `fn` — `fn step(active: Active)
-> Result<StepOutcome, Failed>`. But the generative membrane (an LLM call, a
tool-using agent, any inference-endpoint round-trip inside a body) is naturally
*async*. How does rung host an `async` transition body without breaking the
linear-token guarantees?

**Why it's open.** The whole type machinery — sealed tokens, `!Send`, move
semantics, the auto-injected progress guard — is built around sync `fn`s that
consume a token by value and return the next rung. An `async` body changes the
emitted signature (it returns a `Future`) and raises two coupled unknowns:
1. **Does the guarantee survive `.await`?** A token moved into an async body is
   still moved *once* — linearity should hold. And `!Send` is arguably
   *consistent* with async: a `!Send` token held across an await point pins the
   future to one thread, which is exactly the one-token-one-thread contract (G3).
   But that pinning means the future is itself `!Send` — it cannot run on a
   multi-threaded executor. Whether that composes with a real driver, or forces a
   single-threaded runtime, is the load-bearing question.
2. **What drives an async ladder?** Today the driver is a plain sync loop
   (`match step(token) { .. }`). An async transition needs an async drive — and a
   ladder mixing sync and async transitions needs both.

**Relationship to Q5.** Adjacent but orthogonal. Q5 (fork-join) splits one token
into N concurrent sub-tokens; this is a *single* transition that awaits. You can
have async bodies with no fork-join, or fork-join with sync bodies — they are two
different concurrency questions.

**Most promising angle.** The surfacing case is real and immediate: the first
LLM-in-a-transition fold (outer-loop question resolution, cut #2) deliberately
chose a *blocking* client (`reqwest` blocking / `ureq`) precisely to sidestep
this — a batch resolver has no latency pressure, so a sync body over a blocking
call is honest and correct *for now*. That sidestep is what parks the question
rather than closing it. Two experiments when it ripens:
1. A per-transition `async` marker in the DSL (`async active = |spec| { .. }`),
   emitting `async fn` for that transition and an async `StepOutcome` drive;
   sync and async transitions coexist in one ladder.
2. Verify the `!Send` / await interaction against a real executor — does the
   pinned-future consequence force a single-threaded runtime, or does it compose?

**Provenance.** Surfaced 2026-07-18, choosing the client for the first
deterministic→generative fold. The blocking-client decision is the deliberate
YAGNI park: build the smallest real thing, let it tell us when async earns its
place.

---

## The category-theory map as a question generator

[`RUNG-CT.md`](RUNG-CT.md) is not decoration — each structure it identifies
predicts a feature. This is where the growth questions come from, principled
rather than ad hoc:

| CT structure | Predicts | Question |
|---|---|---|
| Free category | Composition of ladder segments | Q4 |
| Indexed monad | Sequencing / do-notation for driving runs | (new) combinators over `StepOutcome` |
| Dagger | Reversibility | Recovery is already a *partial* dagger — what is a principled reverse transition? |
| Linear logic | A linear substrate | Q3 adjacent |

---

## Triage

- **Blocked externally:** Q3 (true linearity ← the language).
- **Known but costly — park:** Q2 (cross-crate provenance ← sub-crate).
- **Open and ours to push:** Q1 (verifier spike), Q4 (composition), Q5 (fork-join),
  Q6 (genericity), Q7 (async transition bodies — parked pragmatically behind the
  blocking-client choice, but real).

**Highest-information next experiments.**
- To *deepen*: the Q1 Kani/Creusot spike on a single ladder — does a generated
  per-transition contract prove?
- To *grow*: Q4 composition — the free-category structure the theory already
  guarantees is present, and the smallest real step beyond a linear ladder.
