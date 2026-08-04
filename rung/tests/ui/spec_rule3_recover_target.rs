//! rung-props.md §2 rule 3 — a recoverable verdict's `=>` target must be a
//! declared rung.
//!
//! `Stalled => Nonexistent` says the run comes back to a rung that is not on
//! the spine. Rule 2 (a transition naming an undeclared `from`/`to` rung) has
//! no case of its own because the grammar makes it unreachable: every rung of
//! the spine is declared by the hop that introduces it. This is the branch of
//! the same family that a hand-written declaration can actually reach.
//!
//! The intended diagnostic is the macro's own error, quoted in full below.

use rung::ladder;

struct S;
struct L;

ladder!(Bad {
    Start(S) => Working(L) => { Stalled => Nonexistent }
    recover { unstall: Stalled => Working }
});

fn main() {}
