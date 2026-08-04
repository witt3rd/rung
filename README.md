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

```rust,ignore
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

### Why "ladder"?

Because the accurate name would defeat the design. A `ladder` declaration is a
presentation of a free category on a linear graph — but the surface deliberately
never says so. If the syntax required the mathematics, the enforcement would rest
on you restating it correctly, which is the failure the macro exists to remove.
You write rungs and transitions; the construction is what owes the category.

The metaphor also happens to carry the guarantees: rungs are discrete positions
(a rung is an object, inert), you cannot skip one (the category is freely
generated, so a skipping path does not exist to be taken), and you cannot stand
on two at once (composition consumes its input). It even scales — ladders stack
into *towers*, and that turned out to name a real structure rather than a figure
of speech.

Fuller version, including where the metaphor strains, in
[`docs/rung-notes.md`](docs/rung-notes.md) §0.

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

struct Task;
struct Job { step: u32 }
struct Output { steps: u32 }

ladder!(Workflow {
    carry { task_id: String }

    Pending(Task) => Running(Job) => {
        Step -> Running        // continue: build the next Running inline
        | Done(Output)         // terminal, carries a result
    }
} impl {
    // Bodies expand INSIDE the generated `workflow` module, so they use the
    // sealed constructors and refer to types unqualified (Running, StepOutcome…).
    // With a `carry` block, every `::new` also takes the carry; read it with
    // `.carry()` and thread it forward.
    running = |pending| { Running::new(Job { step: 0 }, pending.carry().clone()) },
    step = |running| {
        let n = running.payload.step;
        if n >= 3 {
            return Ok(StepOutcome::Done(Done::new(Output { steps: n })));
        }
        Ok(StepOutcome::Step(Running::new(Job { step: n + 1 }, running.carry().clone())))
    },
});

fn main() {
    // Start with the public entry constructor (only the entry rung's `new` is
    // public), then drive by matching on StepOutcome — plain `pub fn`s, no trait.
    let p = workflow::Pending::new(Task, workflow::Carry { task_id: "t1".into() });
    let mut r = workflow::running(p);
    let out = loop {
        match workflow::step(r) {
            Ok(workflow::StepOutcome::Step(next)) => r = next,   // continue arm
            Ok(workflow::StepOutcome::Done(d)) => break d.into_payload(),
            Err(f) => panic!("{}", f.error),
        }
    };
    assert_eq!(out.steps, 3);
}
```

Only the *entry* rung's constructor is public — every downstream rung's `new` is
module-private, so no outside code can fabricate a mid-ladder token. Omit the
`impl { .. }` block for a type-only declaration (structs and enum, no logic).

> This example is compile-checked and run as a doctest (via `include_str!` in the
> crate root), so it can't silently drift from the macro.

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
  [the non-guarantees](docs/rung-props.md#non-guarantees) for the full list.

## How rung is specified and verified

The documents are not commentary on the implementation — they are part of how it
is built. Every normative claim has a stable identity, a machine-checked place in
a tree, and a recorded answer to *"what test fails if this stops being true?"*

### Two classes of document, named by the rule

**`*-props.md` is normative.** **`*-notes.md` is informative** — how each account
was derived, what was tried and rejected, and what was later withdrawn. Where a
notes file and its props file disagree, **the props file governs**; a claim made
in notes and not in props is not a claim rung makes.

| subject | normative | informative |
|---|---|---|
| the ladder language | [`docs/rung-props.md`](docs/rung-props.md) | [`docs/rung-notes.md`](docs/rung-notes.md) |
| the category | [`docs/rung-ct-props.md`](docs/rung-ct-props.md) | [`docs/rung-ct-notes.md`](docs/rung-ct-notes.md) |
| Het | [`docs/rung-het-props.md`](docs/rung-het-props.md) | [`docs/rung-het-notes.md`](docs/rung-het-notes.md) |

### The conventions the props files follow

- **Identity is the slug, not the number.** A proposition is anchored
  `<a id="g2-sealed-construction" data-parent="guarantees">`. Its decimal number
  is *derived* from the anchor, its `data-parent`, and document order — so
  inserting, removing, or reparenting a proposition cannot break a reference.
  `docs/_props.py fmt` recomputes every number and link text.
- **One slug space across all three documents.** A reference naming another file
  crosses into it; where a claim here touches one there, it links rather than
  restates. That is what lets a categorical proposition cite the guarantee it is
  the content of.
- **A labelled subtree keeps an outside-facing ID.** `G1`–`G14` (guarantees) and
  `J1`–`J2` (design judgments) are flat labels rather than decimals, because they
  are cited from Rust comments and from `trybuild` test *filenames*, where a
  renumbering would break something a slug cannot reach.
- **Vocabulary is closed.** Terms Het retired are refused across every normative
  document, so a superseded word cannot grow back on the next authoring pass.
- **A claim that no machine settles says so.** `rung-props.md` §1–§6 are settled
  by the macro, by rustc, or by a named test; §7 holds the two design judgments
  no machine decides, and they carry no conformance test on purpose.

### The conformance ledger

[`docs/conformance.md`](docs/conformance.md) is **generated, never written**. It
joins every proposition in all three documents to where it is enforced — and,
more usefully, records where it is *not*. Verdicts are `enforced`,
`expressible`, `deferred`, `collides`, `out-of-scope`, and `unclassified`.

The join is deliberately **not** one-to-one, in either direction: a proposition
may have no test, and a guarantee may have no proposition. `unclassified` exists
so an unproven claim reads as a worklist entry rather than a clean bill, and
`collides` — a claim that contradicts a guarantee — must stay empty.

### What runs in CI

| gate | what it catches |
|---|---|
| `cargo test --workspace` | a guarantee that stopped holding |
| `docs/_props.py check` | a duplicate slug, a dangling reference, a stale number, a retired term |
| `docs/_props.py cited` | a Rust comment citing a proposition that no longer exists |
| `docs/_ledger.py check` | an unclassified-away proposition, a hand-edited ledger, an `enforced` row whose test does not exist |

`_ledger.py check` regenerates the ledger and diffs it against disk, so
`conformance.md` cannot be edited by hand and a new proposition cannot be added
without receiving a verdict.

### Two rules that keep the tests honest

- **No guarantee may cite a `compile_fail` doctest as its evidence**
  ([6.2](docs/rung-props.md#no-guarantee-cites-a-compile-fail-doctest)). rustdoc
  ignores the `E0NNN` in a `compile_fail,E0999` fence — and E0999 does not exist.
  Such a test asserts exactly one thing: *this did not compile*. It cannot tell
  the refusal it was written for from a typo or a missing `main`. Refusals are
  pinned by `trybuild` cases whose committed `.stderr` holds the full rendered
  message.
- **A refusal test that cannot fail is not a guarantee**
  ([6.4](docs/rung-props.md#a-refusal-test-that-cannot-fail)). The way to
  establish that a case *can* fail is to make the guarded thing legal and watch
  it go red.

## Further reading

- [`docs/questions/`](docs/questions/) — the open questions: the frontier
  for advancing the language (deepen vs grow). A question resolves only when its
  answer lands in a normative surface. See
  [`INTAKE.md`](docs/questions/INTAKE.md) for how one enters. The questions are
  themselves governed by a Het theory written in the DSL
  ([`rung-std/src/questions.rs`](rung-std/src/questions.rs)), which declares the
  typed dependency vocabulary; `rung-het/tests/questions_of_rung.rs` evaluates
  every decidable sentence of it against the real files.
- [`docs/rung-het-publishing.md`](docs/rung-het-publishing.md) — a brief for an
  outside reviewer on whether Het is publishable mathematics.

Superseded material is under [`docs/.archive/`](docs/.archive/) — the independent
derivation (`CONVERGENCE.md`) and the three-voices loop (`THREE-VOICES.md`).
