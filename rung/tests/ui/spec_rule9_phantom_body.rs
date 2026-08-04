//! rung-props.md §2 rule 9 — every `impl` body names a real transition.
//!
//! A body whose name matches nothing is a body that will never run. The
//! commonest way to write one is to rename a rung and leave the body behind.
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
    step    = |_w| { Ok(StepOutcome::Done(Done::new())) },
    ghost   = |_w| { unreachable!() },
});

fn main() {}
