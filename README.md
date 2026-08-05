# rung 🪜

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

## The other half: theories, judgment, and questions

Everything above declares **arrows** — the legal moves. The second half of the
system declares **sentences**: claims about a model that can be true or false.
That's `theory!`, and it's what `rung-het` and `rung-std` are built on.

A sentence comes in one of two kinds, and the difference is the whole point:

- **decidable** — a machine can settle it. `decidable ids_are_unique = |qs|
  qs.duplicate_ids().is_empty();` No outside needed, no one to ask.
- **judgmental** — settling it needs someone outside. Not just anyone: someone
  with the competence the sentence names, who **did not write the thing being
  judged**. That second condition is structural, not advisory — a judge whose
  provenance overlaps the subject's cannot be qualified, so a verdict on your own
  work is a value you cannot construct.

### Audit, and rectify

Every theory has an **audit** half: sentences evaluated over a model. That's it —
sorts, sentences, and someone to ask when a sentence is judgmental.

The **rectify** half is optional. A theory that governs something *editable* also
declares an edit vocabulary and runs the audit-rectify pass: `audit → propose →
dispose → enact`, where proposing is authorial (you need standing over the thing)
and disposing is judgmental (the judge must be disjoint from the proposal).
`rung_std::questions` does both. `rung_std::principals` is audit-only — it has
sentences and no edits, and that's a complete theory, not a half-finished one.

### Anywhere you ask an outside, you might get a question back

This is the part worth internalising early, because it shapes everything
downstream. When you consult a principal, the answer is a verdict **or** a
matter it raised instead — *"I can't settle this until something else is
settled."* That isn't an error path. It's the ordinary case for anything hard,
and it's why the loop is a loop.

A raised question is opaque to rung. It's a string the theory chose — an issue
number, a filename, whatever — and rung never parses it, orders it, or asks
whether it's well-formed. It carries it from the principal that raised it to the
edge that resumes on it, and nothing else.

Deferral happens in the pool, at the moment of consultation. So it can happen to
**any** judgmental sentence in any theory, pass or no pass.

### What's left holding the work while you wait

Three shapes, and they differ in whether there's anything to hold:

| where the question arose | what's left waiting | held by |
|---|---|---|
| a judgmental *forward* transition in a ladder | the argument, handed back unconsumed as `Suspended<Prev>` | [`rung_std::driver::Park`](rung-std/src/driver.rs) |
| a judgmental *branching* transition | nothing — no residual channel here | — |
| a `theory!` sentence | nothing consumed; a sentence borrows its model | re-consult later |

Only the first case has a linear token that would be lost if dropped, which is
why it's the one with a residual channel and a park. `Park` holds suspended runs
and releases each one when evidence arrives that *its* matter terminated.

It is deliberately stupid. No ordering, no priority, no depth cap, no timeout, no
bound on how many times a run may suspend and resume. Every one of those is a
judgment about which work matters more or how long an answer may take, and rung
declines to make it — that belongs to whatever sits above. A question may take
ten rounds or never terminate, and the park's job is to make that **visible**,
not to resolve it.

It also can't resume anything: it hands the run back, and the ladder's own
pen-gated resume edge does the resuming. And it's in-memory only — whether a
suspended run can survive process death at all is
[Q13](docs/questions/open/q13-suspension-across-process-death.md), open.

### Questions about questions are just more questions

Here's the part that makes the whole thing hold together.

Questions are themselves governed by a theory — `rung_std::questions`, written in
this DSL, with an edit vocabulary and a lifecycle ladder. So auditing the
questions can raise a question. And *that* question is a subject of the same
theory you were already in.

The recursion doesn't climb. It lands back where it started.

That sounds like a curiosity and is actually the load-bearing fact, because it
means **the nesting is type-stable**. One `Park` serves depth 1 and depth 40
alike. If each nested question ascended a level, a depth-*n* suspension would
need a type indexed by *n*, and "unbounded depth" would be not merely unbounded
but impossible to write down — you'd be forced into either a cap (a judgment
about which work matters least) or type erasure (throwing away the seal that
makes any of this worth doing).

It's why nesting can be *normal* rather than exceptional. Answering one question
routinely raises another: Q11 raised Q12, and Q12 took a dependency back on Q11.
That's not a cycle and not a fault — it's what thinking looks like — so the
theory distinguishes it from deadlock rather than flagging it. Only a cycle in
`gate` edges, where each question is *blocked by* the next, is a real stuck
state, because no answer anywhere can free it.

Being a subject of your own theory is not a paradox here. A theory is
**self-governing** — its own decidable sentences audit it — but not
**self-closing**: its judgmental sentences still need an outside. The regress
stops on a decidable well-formedness check that needs no theory to run.

Fuller treatment in [`docs/rung-het-props.md`](docs/rung-het-props.md);
[`docs/composition-notes.md`](docs/composition-notes.md) sketches where this is
going.

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

- **The `*-props.md` files are generated.** Their source is
  [`rung-doctrine/`](rung-doctrine/), and they are written by
  `cargo run -p rung-doctrine --bin render`. Editing one directly does nothing —
  the next render restores it, and CI fails if the two disagree.
- **Identity is the slug, not the number.** A proposition declares a slug and a
  parent; its decimal number is *computed at render time* from its place in the
  tree and appears nowhere in the source. So there is no number to go stale and
  no link text to disagree with its target — inserting, removing or reparenting
  a proposition cannot break a reference, because nothing anywhere stores one.
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

### The conformance record

[`docs/conformance.md`](docs/conformance.md) is a **view of the doctrine**, not a
second record of it. Every column but one is derived: the number from the tree,
the kind from the encoding, the proof from `Kind::Decidable { proof }`.

It was not always. Until recently the record was generated from a curated Python
table that held, per proposition, a verdict and the test establishing it — the
same fact the doctrine holds. The two drifted within a day, and nothing compared
them. Rendering the record from the encoding removes the second copy rather than
adding a check for it.

The one column no machine derives is **mechanism**: *why* a proof is the right
proof for a claim. That is a reading — `establishes_what_it_cites`, judgmental
and unsettled — and the prose is a human's answer standing in until a judge
gives one.

### What runs in CI

| gate | what it catches |
|---|---|
| `cargo test --workspace` | a guarantee that stopped holding |
| `cargo run -p rung-doctrine --bin render -- --check` | a hand-edited `*-props.md` or `conformance.md` — all four are generated |
| `docs/_props.py check` | a stale number or dangling reference, derived **independently** from the rendered markdown — a second opinion on the numbering, from the other side |
| `docs/_props.py cited` | a Rust comment citing a proposition that no longer exists |

`render --check` regenerates all four documents and diffs them against disk, so
none can be edited by hand and a new proposition cannot be added without a kind.

`_props.py` survives for exactly two jobs, and neither duplicates the encoding:
it re-derives every number from the *rendered markdown* rather than from the
source, which makes it an independent second implementation agreeing on all 380;
and it checks that Rust comments citing a slug still resolve, which nothing else
does.

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

## Bootstrapping

The `*-props.md` files are generated from [`rung-doctrine/`](rung-doctrine/), so
the doctrine is data. That is only useful because of what the data says about
each proposition: **what kind of authority could settle it.**

### Five kinds, and they are not a status field

| kind | discharged by | count |
|---|---|---:|
| **decidable** | a proof — a test that fails when the proposition is violated | 124 |
| **judgmental** | a principal, **disjoint** from what it judges | 47 |
| **owed** | an author, with **standing** over it | 3 |
| **signature** | nobody — it declares vocabulary | 59 |
| **rationale** | nobody — it argues, or records a limit | 147 |

The two middle rows are the point. They route to **structurally exclusive**
principals: judgment requires `π(p) ∩ π(a) = ∅` (*you did not write this*),
authorship requires `standing(p, M)` (*you have authority over this*). Opposite
conjuncts of one filter, so no principal is both for the same thing.

Which means the classification is not a taxonomy someone invented. It falls out
of **who is permitted to act**, and a proposition's kind is therefore also its
routing.

### `owed` is "nobody yet"; `judgmental` is "nobody ever"

A status field can say *not implemented*. What it cannot say is that the two go
to different people with different powers.

```text
one-gate-unimplemented   →  #[conditional(..)] is a parse-time refusal,
                            not an encoding
```

That is not waiting on a mathematician. It is waiting on code that does not
exist. Filing it `judgmental` would spend the scarcest resource in the system —
a qualified outside — on work no judge can do.

So: **a judgmental proposition asks a principal a question; an owed one tells an
author what to build.** A test prints the queue.

### A limit is proven by exercising it

The non-guarantees ([§5](docs/rung-props.md#non-guarantees)) look unprovable — a
claim that something is *not* enforced has no satisfying model. It has a proof
anyway: a test that **exercises the gap**.

`G4` is `#[must_use]` and escapable, so the proof escapes it three ways under
`deny(unused_must_use)`. `G8` catches identical-token stalls but not general
non-progress, so the proof is a hundred guard-satisfying rounds converging on
nothing.

These fail when the system gets **stronger** — the only tests here for which a
red run is good news. That matters in both directions: a specification that
*understates* its guarantees is as wrong as one that overstates them, and a limit
someone builds on can close underneath them silently.

### What "bootstrap" means, and does not

Not that the doctrine is *stored* in rung's own format. That is
self-**describing**, and it is what the encoding above achieves.

Bootstrapping is self-**hosting**: rung's own development running through
[the audit-rectify pass](docs/rung-het-props.md#the-pass). A real audit finding
a real defect, an author proposing a fix, a judge disposing of it — with each
step requiring someone who could not have made the previous one.

**That has not happened yet.** Zero judgments have been settled and zero owed
items discharged. The mechanism exists — [`rung-driver`](rung-driver/) builds a
pool from [`docs/population.yaml`](docs/population.yaml), the pass is a ladder,
edits are typed and verified against the source an author actually wrote — and
it has never run on anything that mattered.

Two things stand in the way, and only one is work:

- **[Q14](docs/questions/open/q14-model-principal-provenance.md)** — what
  provenance a model principal carries. Until it is ruled on, a model judge
  either qualifies for nothing or qualifies vacuously.
- **[Q15](docs/questions/open/q15-does-the-pass-suspend.md)** — the pass
  disposes through a *branching* transition, which has no residual channel, so
  it cannot wait for a question it raised.

The honest measure of this project is not 380 propositions encoded or 239 tests
passing. It is **how many defects in rung were found and fixed by the loop
rather than by a person**. That number is zero.

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
