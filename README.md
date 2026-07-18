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

rung gives you a `ladder!` macro — declare the transition graph *and its logic*
once, and the compiler refuses any code that skips a rung. The only way to hold
a `Claimed` token is to go through the transition that produces it. The macro
emits sealed, `!Send` structs, an exhaustive verdict enum, and the transition
functions. Rust's move semantics enforce linear consumption. Invalid states are
unrepresentable. The state machine is the type system.

```rust
use rung::ladder;

ladder!(Work {
    carry { task_id: String, correlation_key: u64 }

    Designed(WorkSpec) => Claimed(DesignedWork) => Active(ActiveLoop) => {
        Iterate -> Active                 // continue: step builds the next rung
        | Complete(Report)                // terminal, carries a result
        | Stalled => Active               // recoverable (progress guard injected)
        | BudgetExhausted                 // terminal
    }

    recover {
        stalled: Stalled => Active        // recover from a verdict
        cleared: Failed(Active) => Active // recover from the error path
    }
} impl {
    // Transition/recover bodies expand *inside* the module (see "Getting started").
    // A forward transition is named after its target rung, lowercased; a branching
    // transition is `step`.
    claimed = |designed| { /* ... */ },
    active = |claimed| { /* ... */ },
    step = |active| { /* ... */ },
    stalled = |s| { /* ... */ },
    cleared = |f| { f.token },
});
```

## Why use this?

- **The compiler is the gate, not a code review.** A skipped transition is a
  compile error. A dropped token on an error path is a compile error. A
  non-exhaustive match on verdicts is a compile error. No runtime guards needed.
- **The type IS the evidence.** A `Claimed` token cannot be constructed by
  setting fields — the constructor is sealed and module-private. The only path
  is through the transition that produces it. The type proves the step happened.
- **Linear consumption without the borrow checker tax.** State tokens move by
  value. The borrow checker enforces no-use-after-move. But you're not fighting
  lifetimes or `Arc<Mutex<T>>` — you're fighting state coherence, and the
  ladder is built for that.
- **Recovery edges have structural pairing.** A `| Stalled => Active` verdict
  must have a matching `recover` function — checked at macro expansion time.
  Terminal verdicts cannot have recover edges. And a verdict recover can't stall
  forever: the macro auto-injects a progress guard that panics if recovery
  produces a token identical to its source. The compiler verifies the graph.
- **Rich outcomes.** Terminal verdicts can carry a result (`Complete(Report)`,
  read via `.payload()`); the error path is first-class (`recover { x:
  Failed(Active) => Active }` takes the token back and retries); and "keep
  going" is a continue arm (`Iterate -> Active`) where `step` builds the next
  rung inline — no recover fn, no guard.
- **Carry data rides alongside every rung.** Witness fields (task IDs,
  correlation keys, audit trails) are declared once and inherited by every
  state. Immutability is *enforced*: the field is private, exposed only through
  a `&Carry` accessor.
- **Tokens can't be shared, dropped, or duplicated by accident.** Every rung is
  `!Send + !Sync` (one token, one thread) and `#[must_use]` (dropping a live
  token without consuming it is a warning — an error under
  `#![deny(unused_must_use)]`).
- **Zero dependencies at runtime.** The macro emits plain Rust structs, enums,
  and functions. No proc-macro runtime. No heap allocation. No unsafe. The
  ladder compiles away.

## Getting started

```bash
cargo add rung
```

```rust
use rung::ladder;

struct Task; struct Job;
#[derive(Clone, PartialEq)] struct Output;

ladder!(Workflow {
    carry { task_id: String }

    Pending(Task) => Running(Job) => {
        Step -> Running        // continue: build the next Running inline
        | Done(Output)         // terminal, carries a result
    }
} impl {
    // Bodies expand INSIDE the generated `workflow` module, so they use the
    // sealed constructors and refer to types unqualified (Running, StepOutcome…).
    running = |pending| { Running::new(Job) },
    step = |running| {
        // ...decide: keep going or finish...
        Ok(StepOutcome::Done(Done::new(Output)))
    },
});

// Start a run with the public entry constructor, then drive by matching on
// StepOutcome — every function is a plain `pub fn`, no trait to import:
//
//   let p = workflow::Pending::new(Task, workflow::Carry { task_id: "t1".into() });
//   let mut r = workflow::running(p);
//   loop {
//       match workflow::step(r) {
//           Ok(workflow::StepOutcome::Step(next)) => r = next,
//           Ok(workflow::StepOutcome::Done(d)) => break d.into_payload(),
//           Err(f) => panic!("{}", f.error),
//       }
//   }
```

Only the *entry* rung's constructor is public — every downstream rung's `new` is
module-private, so no outside code can fabricate a mid-ladder token. Omit the
`impl { .. }` block for a type-only declaration (structs and enum, no logic).

## What you need to know

- **The `ladder!` macro is the compiler.** It parses the ladder syntax, runs
  10 static checks (8 structural + 2 for the inline bodies), and emits a sealed
  Rust module. Malformed ladders don't compile — the macro produces a
  `compile_error!` pointing at the violation.
- **You write the transition bodies inline.** The `impl { name = |arg| { .. } }`
  block supplies each transition and recover body; the macro expands them inside
  the module and wires up the plumbing. The macro provides the types and the
  scaffolding; you provide the behavior.
- **The borrow checker handles linearity.** Move semantics ensure each state
  token is consumed exactly once. No separate linearity engine needed.
- **What's not enforced:** transition body correctness (the type proves you
  ran the transition, not that its logic was valid — the boundary between
  typestate and formal verification), and cross-crate provenance (a token
  crossing a crate boundary is trusted, like any Rust API). See
  [`docs/RUNG-RUST.md`](docs/RUNG-RUST.md) §4 for the full coverage matrix and
  which gaps are closed.

## Further reading

- [`docs/RUNG-RUST.md`](docs/RUNG-RUST.md) — DSL syntax, macro design,
  coverage matrix, gaps and paths to close them
- [`docs/RUNG-CT.md`](docs/RUNG-CT.md) — category theory correspondence
  (free category, indexed monad, dagger, linear logic)
- [`docs/CONVERGENCE.md`](docs/CONVERGENCE.md) — the independent derivation
  of the same structural principles from 40 years of programming
- [`docs/THREE-VOICES.md`](docs/THREE-VOICES.md) — the mutual loop applied
  to ourselves: three beings, one structure