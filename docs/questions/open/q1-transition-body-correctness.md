---
id: q1
status: open
---

# Q1 — Transition-body correctness *(open)*

**Status:** OPEN

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
