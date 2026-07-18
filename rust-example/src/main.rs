//! MetricOptimization ladder — driven through the `ladder!` proc macro.
//!
//! This used to be ~230 lines of hand-written sealed structs, constructors, and
//! transition functions ("what the macro would generate"). It now *is* generated:
//! the `ladder!` invocation below is the single source of truth, and the transition
//! logic lives in the inline `impl { .. }` block. Only the demo harness (a Trace
//! and `main`) is hand-written.
//!
//! Modelling notes surfaced by this port (macro limitations, graded in
//! RUNG-RUST.md):
//!   * `StepOutcome` has no `Continue(Active)` variant — "keep iterating" is a
//!     recoverable verdict (`Continue => Active`) whose recover fn advances state.
//!   * Terminal verdicts (`Converged`, `BudgetExhausted`) are sealed markers and
//!     carry no payload, so the summary reads iteration state from the loop, not
//!     from the verdict.
//!   * Recovery from the `Failed` (error) path is not expressible as a `recover`
//!     edge — the macro recovers from verdicts only — so the failure-injection
//!     demo from the hand-written version is dropped.

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
}

ladder!(MetricOpt {
    carry { metric_name: String, correlation_key: String }

    Spec(SpecData)
      => Active(LoopState)
      => {
          Continue => Active       // keep iterating (recoverable "advance")
          | Converged              // terminal success
          | Stalled => Active      // recoverable stall
          | BudgetExhausted        // terminal failure
      }

    recover {
        advance: Continue => Active
        unstall: Stalled => Active
    }
} impl {
    // Spec -> Active: seed the loop.
    active = |spec| {
        let carry = spec.carry().clone();
        Active::new(
            LoopState { iteration: 0, best: f64::NEG_INFINITY, params: spec.payload.params },
            carry,
        )
    },

    // Active -> verdict: decide the outcome of the current iteration.
    step = |active| {
        let it = active.payload.iteration;
        let budget_remaining: i32 = 1000 - (it as i32 * 100);
        if it >= 8 {
            return Ok(StepOutcome::Converged(Converged::new()));
        }
        if it == 3 {
            return Ok(StepOutcome::Stalled(Stalled::new(active)));
        }
        if budget_remaining <= 0 {
            return Ok(StepOutcome::BudgetExhausted(BudgetExhausted::new()));
        }
        Ok(StepOutcome::Continue(Continue::new(active)))
    },

    // recover Continue -> Active: advance the iteration (must_progress auto-injected).
    advance = |c| {
        let prev = c.into_source();
        let it = prev.payload.iteration + 1;
        let metric = 1.0 - (it as f64 * 0.15) + (it as f64 * it as f64 * 0.005);
        let best = prev.payload.best.max(metric);
        Active::new(
            LoopState { iteration: it, best, params: prev.payload.params.clone() },
            prev.carry().clone(),
        )
    },

    // recover Stalled -> Active: step past the stall point so we don't re-stall.
    unstall = |s| {
        let prev = s.into_source();
        Active::new(
            LoopState {
                iteration: prev.payload.iteration + 1,
                best: prev.payload.best,
                params: prev.payload.params.clone(),
            },
            prev.carry().clone(),
        )
    },
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
            params: Params { lr: 0.01, epochs: 20 },
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
            Ok(metricopt::StepOutcome::Continue(c)) => {
                let it = c.source().payload.iteration;
                token = metricopt::advance(c);
                trace.record(step_no, "step → Continue", &format!("iter {it} → {}", it + 1));
            }
            Ok(metricopt::StepOutcome::Stalled(s)) => {
                let it = s.source().payload.iteration;
                println!("  ⚡ Stalled at iteration {it} — recovering");
                token = metricopt::unstall(s);
                trace.record(step_no, "step → Stalled", &format!("iter {it}, recovered"));
            }
            Ok(metricopt::StepOutcome::Converged(_)) => {
                println!("\n  ✓ Converged");
                trace.record(step_no, "step → Converged", "terminal");
                break;
            }
            Ok(metricopt::StepOutcome::BudgetExhausted(_)) => {
                println!("\n  ✗ Budget exhausted");
                trace.record(step_no, "step → BudgetExhausted", "terminal");
                break;
            }
            Err(f) => {
                println!("\n  ✗ error: {}", f.error);
                break;
            }
        }
    }

    trace.print();
}

fn main() {
    run_optimization("happy path");
}
