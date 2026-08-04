//! SPEC.md §2 rule 3 — a continue arm's target rung must be declared.
//!
//! A branching arm written with `->` (produces) rather than `=>` (recover) is a
//! continue: `step` builds the next rung itself and the `StepOutcome` variant
//! carries it directly. The rung it names has to exist.
//!
//! The intended diagnostic is the macro's own error, quoted in full below.

use rung::ladder;

struct S;

ladder!(Bad {
    Begin(S) => Counting(i32) => { Tick -> Nonexistent | Done }
});

fn main() {}
