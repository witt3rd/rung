//! SPEC.md §2 rule 8 — a `Failed(Rung)` source rung must be declared.
//!
//! `recover { name: Failed(Active) => Active }` recovers from the error path:
//! it takes the unconsumed token back out of `Err(Failed { .. })` and produces
//! the next rung. The rung it names has to exist.
//!
//! The intended diagnostic is the macro's own error, quoted in full below.

use rung::ladder;

struct S;
struct L;

ladder!(Bad {
    Start(S) => Working(L) => { Done }
    recover { clear: Failed(Nonexistent) => Working }
});

fn main() {}
