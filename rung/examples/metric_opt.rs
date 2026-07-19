//! MetricOptimization ladder — driven through the `ladder!` proc macro.
//!
//! This used to be ~230 lines of hand-written sealed structs, constructors, and
//! transition functions ("what the macro would generate"). It now *is* generated:
//! the `ladder!` invocation below is the single source of truth, and the transition
//! logic lives in the inline `impl { .. }` block. Only the demo harness (a Trace
//! and `main`) is hand-written.
//!
//! This example exercises the full macro surface:
//!   * `Iterate -> Active` — a continue arm: `step` advances the loop inline and
//!     hands back the next rung; no recover fn, no guard.
//!   * `Converged(ConvergedReport)` — a terminal verdict carrying a result payload.
//!   * `Stalled => Active` — recoverable verdict (progress guard auto-injected).
//!   * `clear_transient: Failed(Active) => Active` — error-path recovery: `step`
//!     injects one transient failure, returns the token in `Failed`, and the driver
//!     recovers from it (no progress guard on the error path).

use rung::ladder;

#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub lr: f64,
    pub epochs: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpecData {
    pub params: Params,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoopState {
    pub iteration: usize,
    pub best: f64,
    pub params: Params,
    pub failed_once: bool,
}

/// Result carried out *through* the terminal `Converged` verdict.
#[derive(Clone, Debug, PartialEq)]
pub struct ConvergedReport {
    pub iteration: usize,
    pub best: f64,
}

ladder!(MetricOpt {
    carry { metric_name: String, correlation_key: String }

    Spec(SpecData)
      => Active(LoopState)
      => {
          Iterate -> Active             // continue: step advances inline (no recover fn)
          | Converged(ConvergedReport)  // terminal success, carries the result
          | Stalled => Active           // recoverable stall
          | BudgetExhausted             // terminal failure
      }

    recover {
        unstall: Stalled => Active
        clear_transient: Failed(Active) => Active
    }
} impl {
    // Spec -> Active: seed the loop.
    active = |spec| {
        let carry = spec.carry().clone();
        Active::new(
            LoopState {
                iteration: 0,
                best: f64::NEG_INFINITY,
                params: spec.payload.params,
                failed_once: false,
            },
            carry,
        )
    },

    // Active -> verdict: decide the outcome of the current iteration.
    step = |active| {
        let it = active.payload.iteration;
        let budget_remaining: i32 = 1000 - (it as i32 * 100);
        // inject one transient failure at iteration 2 — return the token in `Failed`
        if it == 2 && !active.payload.failed_once {
            let token = Active::new(
                LoopState { failed_once: true, ..active.payload.clone() },
                active.carry().clone(),
            );
            return Err(Failed { token, error: "transient network error".into() });
        }
        if it >= 8 {
            return Ok(StepOutcome::Converged(Converged::new(ConvergedReport {
                iteration: it,
                best: active.payload.best,
            })));
        }
        if it == 3 {
            return Ok(StepOutcome::Stalled(Stalled::new(active)));
        }
        if budget_remaining <= 0 {
            return Ok(StepOutcome::BudgetExhausted(BudgetExhausted::new()));
        }
        // keep iterating: advance the loop inline and hand back the next rung.
        let next_it = it + 1;
        let metric = 1.0 - (next_it as f64 * 0.15) + (next_it as f64 * next_it as f64 * 0.005);
        let best = active.payload.best.max(metric);
        Ok(StepOutcome::Iterate(Active::new(
            LoopState {
                iteration: next_it,
                best,
                params: active.payload.params.clone(),
                failed_once: active.payload.failed_once,
            },
            active.carry().clone(),
        )))
    },

    // recover Stalled -> Active: step past the stall point so we don't re-stall.
    unstall = |s| {
        let prev = s.into_source();
        Active::new(
            LoopState {
                iteration: prev.payload.iteration + 1,
                best: prev.payload.best,
                params: prev.payload.params.clone(),
                failed_once: prev.payload.failed_once,
            },
            prev.carry().clone(),
        )
    },

    // recover from the error path: take the (already-marked) token back and retry.
    // No progress guard here — a transient-error retry may reuse the same state.
    clear_transient = |f| { f.token },
});

// ── demo harness (hand-written) ──────────────────────────────────────────────

struct Trace(Vec<String>);
impl Trace {
    fn new() -> Self {
        Trace(Vec::new())
    }
    fn record(&mut self, step: usize, what: &str, detail: &str) {
        self.0.push(format!("  [{step:02}] {what:>18}  {detail}"));
    }
    fn print(&self) {
        println!("\n── provenance trace ──");
        for e in &self.0 {
            println!("{e}");
        }
        println!("── end trace ──\n");
    }
}

fn run_optimization(label: &str) {
    println!("══════════════════════════════════════════════════");
    println!("  rung — MetricOptimization ({label})");
    println!("══════════════════════════════════════════════════");

    let carry = metricopt::Carry {
        metric_name: "convergence_score".into(),
        correlation_key: "550e8400-e29b".into(),
    };
    let spec = metricopt::Spec::new(
        SpecData {
            params: Params {
                lr: 0.01,
                epochs: 20,
            },
        },
        carry,
    );

    let mut trace = Trace::new();
    trace.record(1, "design→activate", "Spec → Active");
    let mut token = metricopt::active(spec);
    let mut step_no = 1;

    for _ in 0..40 {
        step_no += 1;
        match metricopt::step(token) {
            Ok(metricopt::StepOutcome::Iterate(next)) => {
                let it = next.payload.iteration;
                token = next; // continue arm hands back the next rung directly
                trace.record(step_no, "step → Iterate", &format!("→ iter {it}"));
            }
            Ok(metricopt::StepOutcome::Stalled(s)) => {
                let it = s.source().payload.iteration;
                println!("  ⚡ Stalled at iteration {it} — recovering");
                token = metricopt::unstall(s);
                trace.record(step_no, "step → Stalled", &format!("iter {it}, recovered"));
            }
            Ok(metricopt::StepOutcome::Converged(c)) => {
                let r = c.payload();
                println!(
                    "\n  ✓ Converged at iteration {} (best={:.4})",
                    r.iteration, r.best
                );
                trace.record(
                    step_no,
                    "step → Converged",
                    &format!("iter={}, best={:.4}", r.iteration, r.best),
                );
                break;
            }
            Ok(metricopt::StepOutcome::BudgetExhausted(_)) => {
                println!("\n  ✗ Budget exhausted");
                trace.record(step_no, "step → BudgetExhausted", "terminal");
                break;
            }
            Err(f) => {
                println!("  ⚠ transient error: {} — recovering", f.error);
                token = metricopt::clear_transient(f);
                trace.record(step_no, "step → Failed", "recovered (error path)");
            }
        }
    }

    trace.print();
}

fn main() {
    run_optimization("happy path");
}
