//! End-to-end proof that a `ladder!`-generated module is actually *drivable*:
//! start a run, step it, take the recover edge, and reach a terminal verdict —
//! with real transition bodies, not just type existence.
//!
//! This is the first test that constructs and consumes rung tokens (earlier tests
//! only asserted the types exist). It exercises the sealed constructors (`::new`),
//! the recoverable verdict carrying its source rung, and the `must_progress` guard.

use rung::ladder;

// Payload + witness types the ladder module pulls in via `use super::*`.
#[derive(Debug)]
struct SpecData {
    start: i32,
    target: i32,
}
#[derive(Debug)]
struct LoopState {
    value: i32,
    target: i32,
}

ladder!(Opt {
    carry { budget: u32 }

    Spec(SpecData)
      => Active(LoopState)
      => {
          Iterating => Active   // recoverable: loop back for another step
          | Converged           // terminal success
          | Exhausted           // terminal failure
      }

    recover {
        iterate: Iterating => Active
    }
});

/// The transition logic. Lives *outside* the `opt` module — it can only build
/// rungs through the sealed `::new` constructors, never by struct literal.
struct Optimizer;

impl opt::Transitions for Optimizer {
    // Spec -> Active: seed the loop from the spec.
    fn active(token: opt::Spec) -> opt::Active {
        let carry = token.carry().clone();
        let spec = token.payload;
        opt::Active::new(
            LoopState {
                value: spec.start,
                target: spec.target,
            },
            carry,
        )
    }

    // Active -> { Iterating | Converged | Exhausted }: one step of the loop.
    fn step(token: opt::Active) -> Result<opt::StepOutcome, opt::Failed<opt::Active>> {
        if token.payload.value >= token.payload.target {
            return Ok(opt::StepOutcome::Converged(opt::Converged::new()));
        }
        if token.carry().budget == 0 {
            return Ok(opt::StepOutcome::Exhausted(opt::Exhausted::new()));
        }
        // Not done, budget remains: stall back for another iteration, carrying the
        // current rung into the verdict so recovery has full context to re-enter.
        Ok(opt::StepOutcome::Iterating(opt::Iterating::new(token)))
    }

    // Recover edge Iterating -> Active: make forward progress and re-enter.
    fn iterate(token: opt::Iterating) -> opt::Active {
        let prev = token.into_source();
        let prev_carry = prev.carry().clone();
        let next = opt::Active::new(
            LoopState {
                value: prev.payload.value + 1, // progress
                target: prev.payload.target,
            },
            Opt_carry_spend(&prev_carry),
        );
        // §4.4 guard: assert the recovered token actually advanced.
        opt::must_progress(&prev.payload.value, &next.payload.value);
        next
    }
}

// Spend one unit of budget, producing a fresh (immutable) carry.
#[allow(non_snake_case)]
fn Opt_carry_spend(c: &opt::Carry) -> opt::Carry {
    opt::Carry {
        budget: c.budget.saturating_sub(1),
    }
}

/// Drive a run to a terminal verdict.
use opt::Transitions as _;

fn drive(spec: opt::Spec) -> Result<opt::Converged, opt::Exhausted> {
    let mut active = Optimizer::active(spec);
    loop {
        match Optimizer::step(active) {
            Ok(opt::StepOutcome::Iterating(it)) => active = Optimizer::iterate(it),
            Ok(opt::StepOutcome::Converged(c)) => return Ok(c),
            Ok(opt::StepOutcome::Exhausted(e)) => return Err(e),
            Err(f) => panic!("transition failed: {}", f.error),
        }
    }
}

#[test]
fn drives_to_convergence() {
    // start 0, target 3, ample budget → converges.
    let spec = opt::Spec::new(SpecData { start: 0, target: 3 }, opt::Carry { budget: 10 });
    assert!(drive(spec).is_ok(), "run should converge within budget");
}

#[test]
fn exhausts_budget_when_target_unreachable() {
    // target far away, budget too small → terminates on Exhausted, not forever.
    let spec = opt::Spec::new(
        SpecData {
            start: 0,
            target: 100,
        },
        opt::Carry { budget: 2 },
    );
    assert!(drive(spec).is_err(), "run should exhaust budget and stop");
}

#[test]
#[should_panic(expected = "recovery made no progress")]
fn must_progress_catches_infinite_stall() {
    // A recover body that returns an identical value must trip the §4.4 guard.
    let before = 7i32;
    let after = 7i32;
    opt::must_progress(&before, &after);
}
