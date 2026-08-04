//! rung-props.md §2 rule 1 — two `carry` fields may not share a name.
//!
//! The carry is one struct; two fields of one name is not a shadowing question
//! but an ambiguity the macro must refuse before it emits anything.
//!
//! The intended diagnostic is the macro's own error, quoted in full below.

use rung::ladder;

struct S;
struct L;

ladder!(Bad {
    carry { budget: u32, budget: u32 }

    Start(S) => Working(L) => { Done }
});

fn main() {}
