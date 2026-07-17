# rung ⚒️

A type ladder where the state machine *is* the type system.

## The Problem

You encode state machines by hand in every architecture you build. A work item
transitions through stages — `Spec → Designed → Claimed → Active → Complete`.
Each stage should only be reachable through the transition that produces it.
You enforce this with sealed constructors, private fields, runtime guards,
convention, code review. None of it is the compiler. A skipped step is a
logic error, not a compile error. The state machine lives in comments and hope.

## The Solution

rung gives you a `ladder!` macro — declare the transition graph once, and the
compiler refuses any code that skips a rung. The only way to hold a `Claimed`
token is to call `claim()` on a `Designed`. The macro emits sealed structs and
an exhaustive verdict enum. Rust's move semantics enforce linear consumption.
Invalid states are unrepresentable. The state machine is the type system.

```rust
use rung::ladder;

ladder!(Work {
    carry { task_id: String, correlation_key: u64 }

    Designed(WorkSpec) => Claimed(DesignedWork) => Active(ActiveLoop) => {
        Complete | Stalled => Active | BudgetExhausted
    }

    recover { stalled: Stalled => Active }
});
```

## Why use this?

- **The compiler is the gate, not a code review.** A skipped `claim()` is a
  compile error. A dropped token on an error path is a compile error. A
  non-exhaustive match on verdicts is a compile error. No runtime guards needed.
- **The type IS the evidence.** `ClaimedWork` cannot be constructed by setting
  fields — the constructor is sealed. The only path is through `claim()`. The
  type proves the step happened.
- **Linear consumption without the borrow checker tax.** State tokens move by
  value. The borrow checker enforces no-use-after-move. But you're not fighting
  lifetimes or `Arc<Mutex<T>>` — you're fighting state coherence, and the
  ladder is built for that.
- **Recovery edges have structural pairing.** A `| Stalled => Active` verdict
  must have a matching `recover` function — checked at macro expansion time.
  Terminal verdicts cannot have recover edges. The compiler verifies the graph.
- **Carry data rides alongside every rung.** Witness fields (task IDs,
  correlation keys, audit trails) are declared once and inherited by every
  state. Immutable by convention, structurally shared.
- **Zero dependencies at runtime.** The macro emits plain Rust structs, enums,
  and traits. No proc-macro runtime. No heap allocation. No unsafe. The ladder
  compiles away.

## Getting started

```bash
cargo add rung
```

```rust
use rung::ladder;

ladder!(Workflow {
    carry { task_id: String }
    Pending(Task) => Running(Job) => Done(Output) => { Success | Failed }
});

struct Task; struct Job; struct Output;

// Generated: sealed structs, StepOutcome enum, Transitions trait.
// Implement the trait to provide transition bodies.
struct WorkCtx;
impl workflow::Transitions for WorkCtx {
    fn start(pending: workflow::Pending) -> workflow::Running { /* ... */ }
    fn finish(running: workflow::Running) -> Result<workflow::StepOutcome, workflow::Failed<workflow::Running>> { /* ... */ }
}
```

## What you need to know

- **The `ladder!` macro is the compiler.** It parses the ladder syntax, runs
  8 static checks, and emits a sealed Rust module. Malformed ladders don't
  compile — the macro produces a `compile_error!`.
- **You write the transition bodies.** The macro emits a `Transitions` trait.
  You implement it. The macro provides the types; you provide the behavior.
- **The borrow checker handles linearity.** Move semantics ensure each state
  token is consumed exactly once. No separate linearity engine needed.
- **What's not enforced:** transition body correctness (the type proves you
  called `claim()`, not that the claim was valid), recovery progress
  (liveness, not safety).

## Further reading

- [`docs/RUNG-RUST.md`](docs/RUNG-RUST.md) — DSL syntax, macro design,
  coverage matrix, gaps and paths to close them
- [`docs/RUNG-CT.md`](docs/RUNG-CT.md) — category theory correspondence
  (free category, indexed monad, dagger, linear logic)
- [`docs/CONVERGENCE.md`](docs/CONVERGENCE.md) — the independent derivation
  of the same structural principles from 40 years of programming
- [`docs/THREE-VOICES.md`](docs/THREE-VOICES.md) — the mutual loop applied
  to ourselves: three beings, one structure