//! End-to-end proof that a `ladder!` with an inline `impl { .. }` block is
//! actually *drivable*: start a run, step it, take the recover edge, reach a
//! terminal verdict — with real transition bodies written inline.
//!
//! This exercises the inline-closure form (RUNG-RUST.md §1): bodies expand
//! *inside* the module, so they use the sealed (private) constructors, and the
//! macro auto-injects the `must_progress` guard around the recover body.

use rung::ladder;

// Payload types the ladder module pulls in via `use super::*`.
// `LoopState` needs `Clone + PartialEq` for the auto-injected progress guard.
#[derive(Debug)]
struct SpecData {
    start: i32,
    target: i32,
}
#[derive(Debug, Clone, PartialEq)]
struct LoopState {
    value: i32,
    target: i32,
}
// A result the run returns *through* the terminal `Converged` verdict.
#[derive(Debug, PartialEq)]
struct Report {
    iterations: i32,
}

ladder!(Opt {
    carry { budget: u32 }

    Spec(SpecData)
      => Active(LoopState)
      => {
          Iterating => Active   // recoverable: loop back for another step
          | Converged(Report)   // terminal success, carrying a result payload
          | Exhausted           // terminal failure
      }

    recover {
        iterate: Iterating => Active
    }
} impl {
    // Spec -> Active: seed the loop from the spec. `Active::new` is private to
    // the module — reachable here only because the body expands inside it.
    active = |spec| {
        let carry = spec.carry().clone();
        let s = &spec.payload;
        Active::new(LoopState { value: s.start, target: s.target }, carry)
    },

    // Active -> { Iterating | Converged | Exhausted }: one step of the loop.
    step = |active| {
        if active.payload.value >= active.payload.target {
            return Ok(StepOutcome::Converged(Converged::new(Report {
                iterations: active.payload.value,
            })));
        }
        if active.carry().budget == 0 {
            return Ok(StepOutcome::Exhausted(Exhausted::new()));
        }
        // Carry the current rung into the verdict so recovery has full context.
        Ok(StepOutcome::Iterating(Iterating::new(active)))
    },

    // Recover edge Iterating -> Active. No must_progress call here: the macro
    // wraps this body with the guard automatically (§4.4 enforced).
    iterate = |it| {
        let prev = it.into_source();
        Active::new(
            LoopState {
                value: prev.payload.value + 1, // progress
                target: prev.payload.target,
            },
            Carry {
                budget: prev.carry().budget.saturating_sub(1),
            },
        )
    },
});

/// Drive a run to a terminal verdict — all module `pub fn`s, no trait in scope.
fn drive(spec: opt::Spec) -> Result<opt::Converged, opt::Exhausted> {
    let mut active = opt::active(spec);
    loop {
        match opt::step(active) {
            Ok(opt::StepOutcome::Iterating(it)) => active = opt::iterate(it),
            Ok(opt::StepOutcome::Converged(c)) => return Ok(c),
            Ok(opt::StepOutcome::Exhausted(e)) => return Err(e),
            Err(f) => panic!("transition failed: {}", f.error),
        }
    }
}

#[test]
fn drives_to_convergence() {
    // start 0, target 3, ample budget → converges through recover cycles,
    // and the terminal verdict carries the result payload back out.
    let spec = opt::Spec::new(
        SpecData {
            start: 0,
            target: 3,
        },
        opt::Carry { budget: 10 },
    );
    let converged = match drive(spec) {
        Ok(c) => c,
        Err(_) => panic!("run should converge within budget"),
    };
    assert_eq!(
        converged.payload(),
        &Report { iterations: 3 },
        "Converged verdict should carry the result through to the caller"
    );
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

// Continue arm (`Tick -> Counting`): `step` produces the next rung directly, with
// no recover fn, no guard, no source-carrying. The driver just reassigns.
struct CData;
#[derive(Debug, PartialEq)]
struct CReport {
    n: i32,
}

ladder!(Count {
    Begin(CData) => Counting(i32) => {
        Tick -> Counting      // continue: step builds the next Counting inline
        | Done(CReport)       // terminal, carries the count
    }
} impl {
    counting = |_b| { Counting::new(0) },
    step = |c| {
        if c.payload >= 3 {
            return Ok(StepOutcome::Done(Done::new(CReport { n: c.payload })));
        }
        Ok(StepOutcome::Tick(Counting::new(c.payload + 1))) // next rung, directly
    },
});

#[test]
fn continue_arm_loops_without_a_recover_fn() {
    let mut cur = count::counting(count::Begin::new(CData));
    let done = loop {
        match count::step(cur) {
            Ok(count::StepOutcome::Tick(next)) => cur = next, // no recover call
            Ok(count::StepOutcome::Done(d)) => break d,
            Err(_) => unreachable!("this ladder never fails"),
        }
    };
    assert_eq!(done.payload(), &CReport { n: 3 });
}

#[test]
#[should_panic(expected = "recovery made no progress")]
fn must_progress_guard_panics_on_no_progress() {
    // The guard the macro injects around every recover body, exercised directly.
    let before = 7i32;
    let after = 7i32;
    opt::must_progress(&before, &after);
}

// A ladder whose recover body makes NO progress — proves §4.4 is *auto-injected*:
// `again` never calls `must_progress`, yet driving one recover cycle panics.
struct AState(i32);
#[derive(Clone, PartialEq)]
struct BState(i32);

ladder!(Stuck {
    A(AState) => B(BState) => { Loop => B }
    recover { again: Loop => B }
} impl {
    b = |a| { B::new(BState(a.payload.0)) },
    step = |b| { Ok(StepOutcome::Loop(Loop::new(b))) },   // always loops
    again = |lp| {
        let prev = lp.into_source();
        B::new(BState(prev.payload.0))                    // returns the SAME value
    },
});

// Error-path recovery: `step` fails transiently and returns the token inside
// `Failed`; a `recover { .. : Failed(Working) => Working }` edge recovers it.
// No progress guard is injected on the error path.
struct StartData;
struct WorkState {
    attempts: u32,
}
#[derive(Debug, PartialEq)]
struct Summary {
    attempts: u32,
}

ladder!(Net {
    Start(StartData) => Working(WorkState) => { Done(Summary) }
    recover { clear: Failed(Working) => Working }
} impl {
    working = |_s| { Working::new(WorkState { attempts: 0 }) },
    step = |w| {
        if w.payload.attempts < 2 {
            // transient failure — hand the (advanced) token back in `Failed`
            let next = Working::new(WorkState { attempts: w.payload.attempts + 1 });
            return Err(Failed {
                token: next,
                error: format!("transient failure (attempt {})", w.payload.attempts),
            });
        }
        Ok(StepOutcome::Done(Done::new(Summary { attempts: w.payload.attempts })))
    },
    // recover from the error path: take the unconsumed token back and retry.
    clear = |f| { f.token },
});

fn drive_net(start: net::Start) -> net::Done {
    let mut cur = net::working(start);
    loop {
        match net::step(cur) {
            Ok(net::StepOutcome::Done(d)) => return d,
            Err(f) => cur = net::clear(f), // error-path recovery
        }
    }
}

#[test]
fn recovers_from_the_failed_error_path() {
    let done = drive_net(net::Start::new(StartData));
    assert_eq!(
        done.payload(),
        &Summary { attempts: 2 },
        "should complete after recovering from 2 transient failures"
    );
}

#[test]
#[should_panic(expected = "recovery made no progress")]
fn recover_guard_is_auto_injected() {
    // `Stuck::again` has no must_progress call in its body — the macro injected it.
    let b = stuck::b(stuck::A::new(AState(0)));
    match stuck::step(b) {
        Ok(stuck::StepOutcome::Loop(lp)) => {
            let _ = stuck::again(lp); // guard fires here → panic
        }
        _ => unreachable!(),
    }
}
