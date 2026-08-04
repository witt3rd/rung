//! rung-props.md §2 rule 5 — a `recover` edge's target rung must be declared.
//!
//! Rule 5's other clause — an edge with no matching recover function — is
//! unreachable through the grammar: one `recover { name: V => R }` entry pushes
//! the edge and the function together, so they cannot come apart. The target
//! rung can still be stale, and that is what this case pins.
//!
//! The intended diagnostic is the macro's own error, quoted in full below.

use rung::ladder;

struct S;
struct L;

ladder!(Bad {
    Start(S) => Working(L) => { Stalled => Working }
    recover { unstall: Stalled => Nonexistent }
});

fn main() {}
