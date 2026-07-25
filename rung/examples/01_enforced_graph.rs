//! # Lesson 1 — A graph the compiler enforces
//!
//! **Chaos admitted: none.** This is the deterministic skeleton every later
//! lesson builds on.
//!
//! **What's new.** You declare the states and the legal moves between them
//! *once*, in one place, and the compiler refuses any code that takes an
//! undeclared path. The state machine stops living in comments and hope.
//!
//! **What you now cannot write** — each of these is a *compile* error, not a
//! runtime check (try uncommenting them):
//!
//! * `optimize::Active::new(..)` from out here — only the **entry** rung's
//!   constructor is public. A mid-ladder state cannot be fabricated; the only
//!   way to hold an `Active` is to have gone through `optimize::active`.
//! * using a token after it moved into a transition — a state is consumed
//!   *exactly once* (Rust's move semantics do the enforcing).
//! * a `match` on `StepOutcome` that forgets an arm — the branch set is a sum
//!   type, so a missing case is refused.
//!
//! **The shape.** `Spec → Active → { Converged | BudgetExhausted }`. Two forward
//! hops, then a branch into terminals. No cycles, no failure, nothing
//! unpredictable — those arrive in Lessons 2, 3 and 4.
//!
//! Run: `cargo run -p rung --example 01_enforced_graph`

use rung::ladder;

#[derive(Clone, Debug)]
pub struct Params {
    pub lr: f64,
    pub epochs: usize,
}

/// The working state of one optimization run.
#[derive(Clone, Debug)]
pub struct LoopState {
    pub iteration: usize,
    pub best: f64,
    pub params: Params,
}

/// The result, carried *out through* the terminal verdict — not returned around
/// the side of the graph.
#[derive(Clone, Debug)]
pub struct Report {
    pub iteration: usize,
    pub best: f64,
}

ladder!(Optimize {
    // Witness data every rung inherits. Immutability is enforced: the field is
    // private, readable only through `.carry()`. A transition cannot mutate it.
    carry { metric: String, run_id: String }

    Spec(Params)
      => Active(LoopState)
      => {
          Converged(Report)   // terminal — carries the result out
          | BudgetExhausted   // terminal — no result to carry
      }
} impl {
    // Spec → Active. Named after its target rung, lowercased.
    // The body runs *inside* the generated module, so it may use the sealed
    // constructor `Active::new` — which is exactly what out-here code cannot.
    active = |spec| {
        let carry = spec.carry().clone();
        Active::new(
            LoopState { iteration: 0, best: f64::NEG_INFINITY, params: spec.payload },
            carry,
        )
    },

    // Active → a verdict. One evaluation, one decision, no looping.
    step = |active| {
        let it = active.payload.iteration;
        let budget_remaining: i32 = 1000 - (it as i32 * 100);
        if budget_remaining <= 0 {
            return Ok(StepOutcome::BudgetExhausted(BudgetExhausted::new()));
        }
        let score = 1.0 - (it as f64 * 0.15);
        Ok(StepOutcome::Converged(Converged::new(Report {
            iteration: it,
            best: active.payload.best.max(score),
        })))
    },
});

fn main() {
    println!("═══════════════════════════════════════════════");
    println!("  Lesson 1 — a graph the compiler enforces");
    println!("═══════════════════════════════════════════════\n");

    // A run starts at the entry rung — the only public constructor.
    let spec = optimize::Spec::new(
        Params {
            lr: 0.01,
            epochs: 20,
        },
        optimize::Carry {
            metric: "convergence_score".into(),
            run_id: "run-001".into(),
        },
    );
    println!("  [Spec]    seeded — carry: metric={}", spec.carry().metric);

    // `spec` MOVES here. Using it again below would not compile.
    let active = optimize::active(spec);
    println!("  [Active]  iteration {}", active.payload.iteration);

    // The match must cover every declared verdict.
    match optimize::step(active) {
        Ok(optimize::StepOutcome::Converged(c)) => {
            let r = c.payload();
            println!(
                "  [Verdict] Converged — iter {}, best {:.4}",
                r.iteration, r.best
            );
            // The result rode out *through* the verdict.
            let report = c.into_payload();
            println!(
                "\n  ✓ result recovered from the terminal: best = {:.4}",
                report.best
            );
        }
        Ok(optimize::StepOutcome::BudgetExhausted(_)) => {
            println!("  [Verdict] BudgetExhausted — terminal, no result");
        }
        // Every branching transition can also fail; Lesson 3 is about that path.
        Err(f) => println!("  [Error]   {}", f.error),
    }

    println!("\n  Try breaking it — none of these compile:");
    println!("    optimize::Active::new(..)   // sealed: cannot fabricate a state");
    println!("    optimize::step(active)      // `active` already moved");
    println!("    match .. {{ }}                // missing a verdict arm");
    println!("\n  Next → Lesson 2 adds a cycle, and shows why it cannot spin forever.");
}
