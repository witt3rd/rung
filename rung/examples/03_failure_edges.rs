//! # Lesson 3 — Failure as a declared edge
//!
//! **Chaos admitted: breakage.** Lessons 1 and 2 assumed every transition
//! succeeds. Real work calls things that time out, rate-limit, and reject your
//! credentials. Here failure stops being an exception thrown *past* the type
//! system and becomes an edge *in* the graph.
//!
//! **What's new.** A branching transition already returns
//! `Result<StepOutcome, Failed<Rung>>` — that error arm has been there since
//! Lesson 1, unused. Now we use it, and declare its recovery:
//!
//! ```text
//! recover { retry: Failed(Pending) => Pending }
//! ```
//!
//! Two details carry the whole lesson:
//!
//! 1. **`Failed` hands back the *token*, not a bare error.** Not
//!    `Result<T, String>` — `Failed<Pending>` carries the unconsumed `Pending`
//!    rung itself. That is what makes re-entry possible at all: the recover edge
//!    receives a live token, not a corpse. (This is also *why* it isn't a monad;
//!    see `questions/resolved/q7-…` if you want that rabbit hole.)
//!
//! 2. **The error path is deliberately *unguarded*.** Compare Lesson 2: a
//!    recoverable verdict (`Stalled => Active`) is wrapped in a progress guard
//!    that panics if you don't advance. An error recovery is *not*, because a
//!    retry after a transient blip may legitimately re-send the identical
//!    request. Different intent, different enforcement — and no `Clone + PartialEq`
//!    bound needed on the payload here, unlike Lesson 2.
//!
//! **So what stops an infinite retry?** Not the compiler — you. Boundedness is
//! *data*: `attempts_remaining` lives in the payload, and the ladder must reach
//! a terminal verdict when it hits zero. The graph makes the exhaustion path
//! explicit and unskippable; it does not invent a limit for you.
//!
//! Three runs below exercise all three paths: recover then succeed, a
//! non-retryable error that skips retrying entirely, and exhaustion.
//!
//! Run: `cargo run -p rung --example 03_failure_edges`

use rung::ladder;

pub const MAX_ATTEMPTS: u8 = 3;

/// Which way the (simulated) remote endpoint will misbehave. Deterministic, so
/// the lesson runs identically everywhere — no network, no keys.
#[derive(Clone, Copy, Debug)]
pub enum Scenario {
    /// Fails twice with a transient error, then succeeds.
    TransientThenOk,
    /// Rejects the credentials. Retrying cannot help.
    BadCredentials,
    /// Never comes back up.
    AlwaysDown,
}

#[derive(Clone, Debug)]
pub struct Attempt {
    pub scenario: Scenario,
    pub attempt_no: u8,
    pub attempts_remaining: u8,
    pub last_error: Option<String>,
}

/// Why the ladder gave up — carried out through the terminal verdict.
#[derive(Clone, Debug)]
pub enum Reason {
    /// Non-retryable: retrying would be pointless.
    Fatal(String),
    /// Retryable, but we ran out of attempts.
    Exhausted { last_error: String },
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reason::Fatal(e) => write!(f, "fatal: {e}"),
            Reason::Exhausted { last_error } => {
                write!(f, "out of attempts (last error: {last_error})")
            }
        }
    }
}

/// The simulated endpoint. Classifying errors into *retryable* vs *terminal* is
/// the caller's job — the graph only routes what you classify.
mod endpoint {
    use super::Scenario;

    pub enum Call {
        Ok(String),
        Retryable(String),
        Terminal(String),
    }

    pub fn get(scenario: Scenario, attempt_no: u8) -> Call {
        match scenario {
            Scenario::TransientThenOk if attempt_no >= 3 => Call::Ok("{\"status\":\"ok\"}".into()),
            Scenario::TransientThenOk => Call::Retryable("connection reset".into()),
            Scenario::BadCredentials => Call::Terminal("401 unauthorized".into()),
            Scenario::AlwaysDown => Call::Retryable("503 service unavailable".into()),
        }
    }
}

ladder!(Fetch {
    carry { resource: String }

    // One rung, branching straight to terminals — plus the error edge.
    Pending(Attempt)
      => {
          Fetched(String)   // terminal success, carries the body
          | GaveUp(Reason)  // terminal failure, carries why
      }

    recover {
        // Error-path recovery. No progress guard: a retry may legitimately
        // re-send the same request.
        retry: Failed(Pending) => Pending
    }
} impl {
    step = |pending| {
        // Boundedness is data, not magic: when the budget is gone, the graph
        // *must* land on a terminal.
        if pending.payload.attempts_remaining == 0 {
            let last = pending.payload.last_error.clone()
                .unwrap_or_else(|| "unknown".into());
            return Ok(StepOutcome::GaveUp(GaveUp::new(Reason::Exhausted { last_error: last })));
        }

        match endpoint::get(pending.payload.scenario, pending.payload.attempt_no) {
            endpoint::Call::Ok(body) => Ok(StepOutcome::Fetched(Fetched::new(body))),

            // Terminal: retrying cannot help, so don't. Straight to a verdict.
            endpoint::Call::Terminal(e) => {
                Ok(StepOutcome::GaveUp(GaveUp::new(Reason::Fatal(e))))
            }

            // Retryable: hand the token back through the error edge. The rung
            // rides inside `Failed`, which is what lets `retry` re-enter.
            endpoint::Call::Retryable(e) => {
                let carry = pending.carry().clone();
                let mut next = pending.payload.clone();
                next.last_error = Some(e.clone());
                Err(Failed { token: Pending::new(next, carry), error: e })
            }
        }
    },

    // Spend one attempt and back off. Returning an unchanged token would be
    // fine here — no guard — but we do advance, because that's what bounds it.
    retry = |f| {
        let carry = f.token.carry().clone();
        let mut a = f.token.payload;
        let backoff_ms = 10u64 << a.attempt_no.min(5); // 10, 20, 40 … ms
        a.attempt_no += 1;
        a.attempts_remaining = a.attempts_remaining.saturating_sub(1);
        if a.attempts_remaining > 0 {
            std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
        }
        Pending::new(a, carry)
    },
});

fn drive(label: &str, scenario: Scenario) {
    println!("\n  ── {label} ──");
    let mut token = fetch::Pending::new(
        Attempt {
            scenario,
            attempt_no: 1,
            attempts_remaining: MAX_ATTEMPTS,
            last_error: None,
        },
        fetch::Carry {
            resource: "/v1/report".into(),
        },
    );

    loop {
        let attempt = token.payload.attempt_no;
        match fetch::step(token) {
            Ok(fetch::StepOutcome::Fetched(body)) => {
                println!("    attempt {attempt}: ok — {}", body.into_payload());
                break;
            }
            Ok(fetch::StepOutcome::GaveUp(r)) => {
                println!("    gave up — {}", r.payload());
                break;
            }
            Err(f) => {
                println!("    attempt {attempt}: {} → retrying", f.error);
                token = fetch::retry(f); // the declared error edge
            }
        }
    }
}

fn main() {
    println!("═══════════════════════════════════════════════");
    println!("  Lesson 3 — failure as a declared edge");
    println!("═══════════════════════════════════════════════");

    drive("transient, then succeeds", Scenario::TransientThenOk);
    drive("bad credentials (never retried)", Scenario::BadCredentials);
    drive("always down (attempts exhausted)", Scenario::AlwaysDown);

    println!("\n  Note the middle run: a non-retryable error went straight to a");
    println!("  terminal verdict without spending a single retry. Classification");
    println!("  is yours; the graph only routes what you classify.");
    println!("\n  Next → Lesson 4: the verb itself becomes unpredictable.");
}
