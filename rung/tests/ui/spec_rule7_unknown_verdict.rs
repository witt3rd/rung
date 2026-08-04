//! rung-props.md §2 rule 7, and G7 — a `recover` edge names a declared verdict.
//!
//! The edge's left-hand side is a verdict of some transition. Naming one that
//! appears nowhere is the stale-rename case, and it is silent unless refused.
//!
//! The intended diagnostic is the macro's own error, quoted in full below.

use rung::ladder;

struct S;
struct L;

ladder!(Bad {
    Start(S) => Working(L) => { Done }
    recover { unstall: Stalled => Working }
});

fn main() {}
