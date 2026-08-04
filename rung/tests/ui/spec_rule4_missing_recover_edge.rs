//! rung-props.md §2 rule 4, and G7 — a recoverable verdict needs a `recover` edge.
//!
//! `Stalled => Working` says the run comes back; without a matching edge there
//! is no function to come back through, and the verdict would be a terminal
//! wearing a recoverable's syntax. Continue arms (`->`) are exempt: they carry
//! their target rung live and need no recover fn.
//!
//! The intended diagnostic is the macro's own error, quoted in full below.

use rung::ladder;

struct S;
struct L;

ladder!(Bad {
    Start(S) => Working(L) => { Stalled => Working }
});

fn main() {}
