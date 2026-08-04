//! rung-props.md 1.41 — a transition carries **at most one** gate marker.
//!
//! Het's four gates are alternatives, not a set
//! (rung-het-props.md#four-gates): an operation is decidable, or judgmental, or
//! authorial, or conditional. Two markers on one transition would ask for two
//! second parameters and two prologues, and would claim the arrow is settled
//! two ways at once.
//!
//! The intended diagnostic is the macro's own error.

use rung::ladder;
use rung::Role;

struct SpecData;
struct LoopState;

struct Reviewer;
impl Role for Reviewer {
    const NAME: &'static str = "reviewer";
}
struct Curator;
impl Role for Curator {
    const NAME: &'static str = "curator";
}

ladder!(Bad {
    Spec(SpecData)
        => #[judgmental(Reviewer)] #[authorial(Curator)] Active(LoopState)
        => { Done }
});

fn main() {}
