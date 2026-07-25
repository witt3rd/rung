# Lessons

These are **lessons, meant to be read in order** — not a grab-bag of samples.
Cargo calls the directory `examples/` (so `cargo run --example …` just works),
but the numbering is the curriculum.

```bash
cargo run -p rung --example 01_enforced_graph
cargo run -p rung --example 02_terminating_loops
cargo run -p rung --example 03_failure_edges
cargo run -p rung --example 04_nondeterminism   # runs with no config
```

Every lesson runs with **no setup**. Lesson 4 will use a real language model if
you give it one, and degrades to a heuristic if you don't.

## The through-line

rung's subject is **containment**: build a graph whose legal paths are enforced,
then progressively admit messier things — repetition, failure, unpredictability —
*without losing the enforcement*. Each lesson admits more chaos and shows the
shape still holds.

| # | Lesson | Chaos admitted | What you gain |
|---|---|---|---|
| 1 | [A graph the compiler enforces](01_enforced_graph.rs) | *none* | illegal paths don't compile |
| 2 | [Loops that must terminate](02_terminating_loops.rs) | unbounded repetition | a cycle that's structurally well-founded |
| 3 | [Failure as a declared edge](03_failure_edges.rs) | breakage | failure routes *through* the graph, not around it |
| 4 | [Non-determinism, contained](04_nondeterminism.rs) | unpredictability | an unpredictable verb still can't corrupt the graph |

## The one invariant

Stated once, then demonstrated four times under increasing difficulty:

> Whatever you admit into a transition body — a loop, a failure, a language model
> — it can only **select among declared edges**. It can never invent a state.

You will meet that rule as four different refusals: a sealed constructor (L1), a
progress guard (L2), an error edge that hands the token back (L3), and a model
that may pick an edge but not mint a state (L4). They are the same rule each
time. Only after you've felt it four times is it worth naming — it's the
free-category axiom, and [`docs/RUNG-CT.md`](../../docs/RUNG-CT.md) names it.

**The category theory is deliberately not a prerequisite.** The law in this
library was discovered from the inside — a compiler refusal came first, the
theory named it afterward. The lessons reproduce that order on purpose. Read
RUNG-CT when you're curious *why* it works, not before you can use it.

## Where it ends

Lesson 4 stops at a live edge, not a conclusion. Its `evaluate` calls the model
once with no retry — while Lesson 3 built a perfectly good retry ladder. Nesting
one inside the other (a ladder inside another ladder's transition body) is
**composition**, and that is an *open research question*, not a feature you were
supposed to find. Async transition bodies are another.

See [`docs/questions/`](../../docs/questions/) — Q4 (composition) and Q8 (async).
The curriculum ends where the research starts.
