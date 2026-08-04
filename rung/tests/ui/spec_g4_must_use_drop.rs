//! rung-props.md G4 — no silent drop.
//!
//! Every generated token carries `#[must_use]`. Rust types are affine, so move
//! semantics give "consumed at most once"; `#[must_use]` supplies the other
//! half, "at least once". Dropping a token in statement position is a warning,
//! and an error under `#![deny(unused_must_use)]`.
//!
//! `Converged` is a terminal verdict with no `impl` block, so it is publicly
//! constructible — the only thing under test here is the attribute. If it were
//! ever dropped from the macro's emit, this file would compile.
//!
//! The intended diagnostic is the `unused_must_use` lint, denied.

#![deny(unused_must_use)]

use rung::ladder;

struct SpecData;
struct LoopData;

ladder!(Demo {
    Spec(SpecData) => Active(LoopData) => { Converged | Stalled => Active }
    recover { stalled: Stalled => Active }
});

fn main() {
    demo::Converged::new();
}
