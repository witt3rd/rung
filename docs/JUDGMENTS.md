# Design Judgments

*Normative rules for the `ladder!` macro live in `SPEC.md`. This document
holds the design judgments that govern how rung is used — questions the
compiler cannot answer for you.*

---

## J1 — Where does the tower bottom out?

A rung ladder should terminate where **structural enforcement stops buying
correctness gains**.

The floor of a tower is not defined by the line between "our code" and "a
library." That line is arbitrary — hermes-agent is not our code, and yet
inner-loop models its inner loop as a rung ladder. The floor is not defined
by the boundary between user space and kernel space either; you could keep
extending the tower through syscalls to hardware interrupts. The question is
not ownership, it is leverage.

Ask: *would a rung ladder over the states below this point catch any wrong
transition that the existing infrastructure does not already catch?*

If the answer is no — if the external code (a library, the OS, a protocol
implementation) already handles its own state correctly, and a ladder over it
would add ceremony without catching anything the type system does not already
enforce — then the tower terminates here.

**The principled floor is where structural enforcement stops buying
correctness gains. Everything above that line is ours to model; everything
below it is someone else's type system doing its job.**

Worked example: `raw_call` in `rung-std::llm` is a plain function — one
blocking HTTP POST, `Result<String, RawCallError>`. There are no wrong state
transitions to prevent below it; `reqwest` already handles the I/O correctly.
The ladder above (`LlmCall`) is where states live: the counter check, the
attempt in flight, the terminal verdicts. That is the right floor.

---

## J2 — What belongs in rung-std?

A ladder belongs in `rung-std` when it satisfies two conditions:

1. **It recurs across unrelated domains.** Independent projects would
   otherwise rediscover the same shape — often collapsing rungs that should
   be distinct, or hiding retry logic inside a single morphism body that
   should be two rungs.

2. **The canonical statement is better than any project's derivation.** A
   project that needs this computation is better served by depending on the
   correct formulation than by writing their own. The value is in the shape
   — the right rung boundaries, the right terminal verdicts — not in the
   implementation details.

**The test:** would a project that needed this computation be better off
depending on the canonical statement than deriving their own? If yes, it
belongs in rung-std. If the ladder carries vocabulary that only makes sense
in one domain, it stays in that project.

**Corollary — rung-std is not a kitchen sink.** A ladder that is merely
*useful* does not earn a place in rung-std. The bar is *recurrent and
domain-generic*. A ladder that happens to appear in two projects but carries
domain-specific names should be abstracted at the domain level, not elevated
to rung-std.

Worked examples:

- `LlmCall` (Pending → Calling → {Success | AuthError | Exhausted}) **belongs
  in rung-std** — the bounded-retry + terminal-classification shape recurs
  wherever an LLM is called, across every project in the keiretsu and beyond.
  GL's collapsed single-rung version is a regressed form of the canonical
  two-rung statement; the stdlib is the correction.

- `InnerLoop` (Idle → Calling → {EndTurn | MaxIterations | ...}) **may belong
  in rung-std** — the agentic turn loop shape recurs across any framework that
  drives tool-calling agents. Worth watching whether a second independent
  derivation confirms the shape.

- An audit ladder for a specific register schema **does not belong** — it
  carries domain vocabulary (finding types, charter boundaries) that is
  meaningful only to garden-ladders.

---

*These judgments are earned through use, not derived from first principles.
Amend them when a new case does not fit cleanly.*
