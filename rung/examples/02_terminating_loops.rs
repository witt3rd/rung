//! # Lesson 2 — Loops that must terminate
//!
//! **Chaos admitted: unbounded repetition.** Lesson 1's graph had no cycles, so
//! it could not fail to finish. Now we add one — and the graph still can't spin
//! forever.
//!
//! **What's new.** Two different ways to go *back* to a rung you've already been
//! on, which look similar and mean opposite things:
//!
//! | form | who builds the next rung | guarded? | means |
//! |---|---|---|---|
//! | `Iterate -> Active` (continue arm) | `step`, inline | no | normal progress |
//! | `Stalled => Active` (recoverable verdict) | a paired `recover` fn | **yes** | something went wrong; re-enter |
//!
//! The continue arm is the cheap one: `step` builds the next rung itself and
//! hands it back. Nothing to pair, nothing to guard.
//!
//! The recoverable verdict is the interesting one. Declaring `Stalled => Active`
//! *obliges* you to write a matching `recover` function — omitting it is a
//! compile error. And the macro wraps that function in a **progress guard**: the
//! rung it returns must differ from the one that stalled, or it panics. So a
//! stall→recover→stall cycle cannot repeat identically forever. Termination
//! stops being a thing you hope for.
//!
//! The second ladder below (`stuck`) deliberately recovers *without* advancing,
//! so you can watch the guard refuse it.
//!
//! Run: `cargo run -p rung --example 02_terminating_loops`

use rung::ladder;

#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub lr: f64,
}

/// The progress guard compares the recovered rung's payload against its source,
/// so this needs `Clone + PartialEq`.
#[derive(Clone, Debug, PartialEq)]
pub struct LoopState {
    pub iteration: usize,
    pub best: f64,
    pub params: Params,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub iteration: usize,
    pub best: f64,
}

ladder!(Optimize {
    carry { metric: String }

    Spec(Params)
      => Active(LoopState)
      => {
          Iterate -> Active     // continue arm — step advances inline, no guard
          | Stalled => Active   // recoverable verdict — paired + guarded recover
          | Converged(Report)
          | BudgetExhausted
      }

    recover {
        // Required by `Stalled => Active`. Omit it and the ladder won't compile.
        unstall: Stalled => Active
    }
} impl {
    active = |spec| {
        let carry = spec.carry().clone();
        Active::new(
            LoopState { iteration: 0, best: f64::NEG_INFINITY, params: spec.payload },
            carry,
        )
    },

    step = |active| {
        let it = active.payload.iteration;
        if it >= 6 {
            return Ok(StepOutcome::Converged(Converged::new(Report {
                iteration: it,
                best: active.payload.best,
            })));
        }
        if (1000 - (it as i32 * 100)) <= 0 {
            return Ok(StepOutcome::BudgetExhausted(BudgetExhausted::new()));
        }
        // Stall once, at iteration 3, to exercise the guarded path.
        if it == 3 {
            // The verdict carries the rung it came from — that's what gives the
            // recover fn (and the guard) something to compare against.
            return Ok(StepOutcome::Stalled(Stalled::new(active)));
        }
        // Normal progress: build the next rung right here and hand it back.
        let next = it + 1;
        let score = 1.0 - (next as f64 * 0.15) + (next as f64 * next as f64 * 0.005);
        Ok(StepOutcome::Iterate(Active::new(
            LoopState {
                iteration: next,
                best: active.payload.best.max(score),
                params: active.payload.params.clone(),
            },
            active.carry().clone(),
        )))
    },

    // Recover from the stall. Note there is no `must_progress` call here — the
    // macro injects it around this body. Returning an unchanged rung panics.
    unstall = |s| {
        let prev = s.into_source();
        Active::new(
            LoopState {
                iteration: prev.payload.iteration + 1, // ← the progress
                best: prev.payload.best,
                params: prev.payload.params.clone(),
            },
            prev.carry().clone(),
        )
    },
});

// ── A ladder that refuses to advance, so the guard has something to catch ─────

ladder!(Stuck {
    Begin(u32) => Spin(u32) => { Again => Spin }
    recover { again: Again => Spin }
} impl {
    spin = |b| { Spin::new(b.payload) },
    step = |s| { Ok(StepOutcome::Again(Again::new(s))) },
    // Hands back exactly what it received. This is the infinite-stall bug.
    again = |a| { let prev = a.into_source(); Spin::new(prev.payload) },
});

fn main() {
    println!("═══════════════════════════════════════════════");
    println!("  Lesson 2 — loops that must terminate");
    println!("═══════════════════════════════════════════════\n");

    let spec = optimize::Spec::new(
        Params { lr: 0.01 },
        optimize::Carry {
            metric: "convergence_score".into(),
        },
    );
    let mut token = optimize::active(spec);

    loop {
        match optimize::step(token) {
            // Continue arm: the next rung arrives directly. Just re-bind it.
            Ok(optimize::StepOutcome::Iterate(next)) => {
                println!("  [Iterate]  → iteration {}", next.payload.iteration);
                token = next;
            }
            // Recoverable verdict: route through the guarded recover fn.
            Ok(optimize::StepOutcome::Stalled(s)) => {
                let at = s.source().payload.iteration;
                token = optimize::unstall(s);
                println!(
                    "  [Stalled]  at iteration {at} → recovered to {}",
                    token.payload.iteration
                );
            }
            Ok(optimize::StepOutcome::Converged(c)) => {
                let r = c.payload();
                println!(
                    "\n  ✓ Converged at iteration {} (best {:.4})",
                    r.iteration, r.best
                );
                break;
            }
            Ok(optimize::StepOutcome::BudgetExhausted(_)) => {
                println!("\n  ✗ Budget exhausted");
                break;
            }
            Err(f) => {
                println!("\n  ✗ {}", f.error);
                break;
            }
        }
    }

    // ── Watch the guard refuse a no-progress recovery ────────────────────────
    println!("\n  Now the guard, caught in the act:");
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {})); // keep the output clean
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let b = stuck::spin(stuck::Begin::new(7));
        match stuck::step(b) {
            Ok(stuck::StepOutcome::Again(a)) => {
                let _ = stuck::again(a); // recovers to the *same* value → guard fires
            }
            Err(_) => unreachable!(),
        }
    }));
    std::panic::set_hook(hook);

    match outcome {
        Err(e) => {
            // `assert!` with a literal panics with `&'static str`; with format
            // args it's a `String`. Check both.
            let msg = e
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panicked".into());
            println!("    guard fired: {msg}");
            println!("    → an identical recovery cannot repeat forever.");
        }
        Ok(()) => println!("    (guard did not fire — that would be a bug)"),
    }

    println!("\n  Next → Lesson 3: things break. Failure becomes a declared edge.");
}
