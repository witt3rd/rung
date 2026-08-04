//! SPEC.md G2 — no external fabrication.
//!
//! With an inline `impl { .. }` block, only the *entry* rung's `new` is public;
//! every downstream rung's constructor is private to the generated module. So a
//! mid-ladder token cannot be minted from outside — it can only be obtained by
//! traversing an arrow.
//!
//! The intended diagnostic is **E0624** — a private associated function.
//!
//! The doc-comment version of this example lives in `rung/src/lib.rs`. It is
//! prose as well as a test; this file is the assertion, because rustdoc does
//! not verify the error code on a `compile_fail` block.

use rung::ladder;

struct SpecData;

#[derive(Clone, PartialEq)]
struct LoopData;

ladder!(Demo {
    Spec(SpecData) => Active(LoopData) => { Done | Retry => Active }
    recover { retry: Retry => Active }
} impl {
    active = |_spec| { Active::new(LoopData) },
    step   = |_active| { Ok(StepOutcome::Done(Done::new())) },
    retry  = |retry| { retry.into_source() },
});

fn main() {
    // `Active::new` is private to `demo`. There is no term for "a mid-ladder
    // rung I made myself".
    let _ = demo::Active::new(LoopData);
}
