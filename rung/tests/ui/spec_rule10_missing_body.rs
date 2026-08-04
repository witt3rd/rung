//! rung-props.md §2 rule 10 — an `impl` block supplies every body.
//!
//! Rules 9 and 10 are the two directions of the same correspondence: no phantom
//! bodies, and no gaps. A partial `impl` block would emit a module missing a
//! transition, which reads at the call site as a name that does not exist.
//!
//! The intended diagnostic is the macro's own error, quoted in full below.

use rung::ladder;

struct S;
#[derive(Debug)]
struct L;

ladder!(Bad {
    carry { budget: u32 }

    Start(S) => Working(L) => { Done }
} impl {
    working = |start| { Working::new(L, start.carry().clone()) },
});

fn main() {}
