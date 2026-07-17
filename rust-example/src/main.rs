#![allow(dead_code, unused_assignments)]

// MetricOptimization ladder — hand-written Rust demonstration.
//
// This is what the `ladder!` proc macro would generate. The pattern:
// - Every rung type has a private seal field — external construction is impossible.
// - Transition functions take the previous rung BY VALUE — the borrow checker
//   enforces linear consumption. No `Arc`, no `Mutex`, no lifetimes.
// - The error path returns the unconsumed token inside `Failed<Prev>`.
// - Verdict branching uses an enum — exhaustive match enforced at compile time.

mod metric_opt {
    use std::fmt;

    // ── carry (witness) data ─────────────────────────────────────────────────

    #[derive(Clone, Debug)]
    pub struct Carry {
        pub metric_name: String,
        pub correlation_key: String,
    }

    // ── params ───────────────────────────────────────────────────────────────

    #[derive(Clone, Debug)]
    pub struct Params {
        pub lr: f64,
        pub epochs: usize,
    }

    // ── state tokens (private seal — external construction impossible) ───────

    pub struct Spec {
        _seal: (),
        pub params: Params,
        inject_failure: bool,
        pub carry: Carry,
    }

    pub struct Active {
        _seal: (),
        pub iteration: usize,
        pub best_metric: Option<f64>,
        pub params: Params,
        inject_failure: bool,
        pub carry: Carry,
    }

    pub struct Converged {
        pub iteration: usize,
        pub metric: f64,
        pub carry: Carry,
    }

    pub struct Stalled {
        pub iteration: usize,
        pub metric: f64,
        pub stagnation: usize,
        pub params: Params,
        inject_failure: bool,
        pub carry: Carry,
    }

    pub struct BudgetExhausted {
        pub iteration: usize,
        pub budget_remaining: i32,
        pub carry: Carry,
    }

    // ── error payload — carries the unconsumed token ─────────────────────────

    pub struct Failed<Prev> {
        pub token: Prev,
        pub error: String,
    }

    // ── step outcome enum (exhaustive match enforced by compiler) ─────────────

    pub enum StepOutcome {
        Continue(Active),
        Converged(Converged),
        Stalled(Stalled),
        BudgetExhausted(BudgetExhausted),
    }

    // ── provenance trace ─────────────────────────────────────────────────────

    #[derive(Clone)]
    pub struct TraceEntry {
        pub step: usize,
        pub transition: String,
        pub rung: String,
        pub outcome: String,
        pub detail: String,
    }

    impl fmt::Display for TraceEntry {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "  [{:02}] {:>12}  {:>6}  {}",
                self.step, self.transition, self.rung, self.outcome
            )?;
            if !self.detail.is_empty() {
                write!(f, " — {}", self.detail)?;
            }
            Ok(())
        }
    }

    pub struct Trace(Vec<TraceEntry>);

    impl Trace {
        pub fn new() -> Self {
            Trace(Vec::new())
        }

        pub fn record(
            &mut self,
            step: usize,
            transition: &str,
            rung: &str,
            outcome: &str,
            detail: &str,
        ) {
            self.0.push(TraceEntry {
                step,
                transition: transition.into(),
                rung: rung.into(),
                outcome: outcome.into(),
                detail: detail.into(),
            });
        }

        pub fn print(&self) {
            println!("\n── provenance trace ──");
            for entry in &self.0 {
                println!("{entry}");
            }
            println!("── end trace ──\n");
        }
    }

    // ── sealed constructors ──────────────────────────────────────────────────

    pub fn design(params: Params, inject_failure: bool, carry: Carry) -> Spec {
        Spec {
            _seal: (),
            params,
            inject_failure,
            carry,
        }
    }

    pub fn activate(spec: Spec) -> Active {
        // spec is MOVED — consumed. Cannot be used after this call.
        Active {
            _seal: (),
            iteration: 0,
            best_metric: None,
            params: spec.params,
            inject_failure: spec.inject_failure,
            carry: spec.carry,
        }
    }

    pub fn step(active: Active) -> Result<StepOutcome, Failed<Active>> {
        let it = active.iteration + 1;

        // failure injection
        if it == 2 && active.inject_failure {
            return Err(Failed {
                token: active, // returned unconsumed
                error: "transient network error".into(),
            });
        }

        let metric = 1.0 - (it as f64 * 0.15) + (it as f64 * it as f64 * 0.005);
        let improved = active.best_metric.map_or(true, |prev| metric > prev);
        let best = active.best_metric.map_or(metric, |prev| prev.max(metric));
        let converged = it >= 8;
        let budget_remaining: i32 = 1000 - (it as i32 * 100);

        if converged {
            Ok(StepOutcome::Converged(Converged {
                iteration: it,
                metric,
                carry: active.carry,
            }))
        } else if it >= 3 && !improved {
            Ok(StepOutcome::Stalled(Stalled {
                iteration: it,
                metric,
                stagnation: it - 2,
                params: active.params,
                inject_failure: active.inject_failure,
                carry: active.carry,
            }))
        } else if budget_remaining <= 0 {
            Ok(StepOutcome::BudgetExhausted(BudgetExhausted {
                iteration: it,
                budget_remaining,
                carry: active.carry,
            }))
        } else {
            Ok(StepOutcome::Continue(Active {
                _seal: (),
                iteration: it,
                best_metric: Some(best),
                params: active.params,
                inject_failure: active.inject_failure,
                carry: active.carry,
            }))
        }
    }

    // ── recovery functions ───────────────────────────────────────────────────

    pub fn recover_step_failed(failed: Failed<Active>) -> Active {
        let mut active = failed.token;
        active.inject_failure = false; // one-shot
        active
    }

    pub fn recover_stalled(stalled: Stalled) -> Active {
        Active {
            _seal: (),
            iteration: stalled.iteration,
            best_metric: Some(stalled.metric),
            params: stalled.params,
            inject_failure: stalled.inject_failure,
            carry: stalled.carry,
        }
    }
}

// ── run the loop ─────────────────────────────────────────────────────────────

use metric_opt::*;

fn run_optimization(label: &str, inject_failure: bool) {
    println!("══════════════════════════════════════════════════");
    println!("  rung — MetricOptimization ({label})");
    println!("══════════════════════════════════════════════════");

    let carry = Carry {
        metric_name: "convergence_score".into(),
        correlation_key: "550e8400-e29b".into(),
    };
    let mut trace = Trace::new();
    let mut step_no = 0;

    let params = Params {
        lr: 0.01,
        epochs: 20,
    };

    // design
    let spec = design(params, inject_failure, carry);
    trace.record(
        1,
        "design",
        "→",
        "ok",
        &format!("Spec(trace_id={:p})", &spec),
    );

    // activate
    let mut token = activate(spec);
    step_no = 2;
    trace.record(step_no, "activate", "Spec", "→ ok", "Active");

    // loop
    let max_iter = 20;
    for _ in 0..max_iter {
        step_no += 1;
        match step(token) {
            Ok(StepOutcome::Continue(active)) => {
                trace.record(
                    step_no,
                    "step",
                    "Active",
                    "→ ok",
                    &format!("iter={}", active.iteration),
                );
                token = active;
            }
            Ok(StepOutcome::Converged(c)) => {
                trace.record(
                    step_no,
                    "step",
                    "Active",
                    "→ Converged",
                    &format!("iter={}, metric={:.4}", c.iteration, c.metric),
                );
                println!(
                    "\n  ✓ Converged after {} iterations (metric={:.4})",
                    c.iteration, c.metric
                );
                break;
            }
            Ok(StepOutcome::Stalled(s)) => {
                trace.record(
                    step_no,
                    "step",
                    "Active",
                    "→ Stalled",
                    &format!("iter={}", s.iteration),
                );
                println!("  ⚡ Stalled at iteration {} — recovering", s.iteration);
                token = recover_stalled(s);
                step_no += 1;
                trace.record(step_no, "recover:stalled", "→", "Active", "");
            }
            Ok(StepOutcome::BudgetExhausted(b)) => {
                trace.record(
                    step_no,
                    "step",
                    "Active",
                    "→ BudgetExhausted",
                    &format!("iter={}", b.iteration),
                );
                println!("\n  ✗ Budget exhausted at iteration {}", b.iteration);
                break;
            }
            Err(failed) => {
                trace.record(step_no, "step", "Active", "→ err", &failed.error);
                token = recover_step_failed(failed);
                step_no += 1;
                trace.record(step_no, "recover:failed", "→", "Active", "");
            }
        }
    }

    trace.print();
}

fn main() {
    run_optimization("happy path", false);
    println!();
    run_optimization("with failure", true);
}
