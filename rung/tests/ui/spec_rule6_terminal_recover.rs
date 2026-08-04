//! rung-props.md §2 rule 6, and G7 — a terminal verdict has no `recover` edge.
//!
//! A terminal verdict ends the run. An edge off it would be a resurrection, and
//! the emitted recover function would take a source rung the terminal verdict
//! does not carry.
//!
//! The intended diagnostic is the macro's own error, quoted in full below.

use rung::ladder;

struct S;
struct L;

ladder!(Bad {
    Start(S) => Working(L) => { Done }
    recover { revive: Done => Working }
});

fn main() {}
