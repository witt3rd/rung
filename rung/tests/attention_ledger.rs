//! AttentionLedger ladder — the outer-loop's credit-assignment discipline,
//! expressed in rung.
//!
//! The problem it closes: in the outer-loop bets registry, a bet can move from
//! `active/` to `resolved/` without the credit-assignment step ever happening.
//! Nothing structurally prevents it — only discipline, held by memory. That's
//! advisory. This ladder makes the skip a compile error.
//!
//! The ladder:
//!
//!   Surfaced(Advantage) => Decided(Action) => Assessed(CreditRecord) => {
//!       EstimateHeld | EstimateMissed
//!   }
//!
//! No recover edge. No path from Decided to any terminal state without passing
//! through Assessed. The compiler refuses the skip.
//!
//! What this enforces:
//!   - A bet cannot "resolve" without an Assessed token existing.
//!   - An Assessed token can only come from a Decided token via `assessed()`.
//!   - A Decided token can only come from a Surfaced token via `decided()`.
//!   - Surfaced(Advantage) captures the baseline comparison at surface time:
//!     not "is this important" but "is this better than autopilot default."
//!
//! What this does NOT enforce (same honest limits as everywhere in rung):
//!   - That the Advantage estimate was honest.
//!   - That the CreditRecord reflection was genuine.
//!   - That your lived attention actually followed the decision.
//!   The type proves the step was called. It doesn't prove the spirit was present.

use rung::ladder;

// ── payload types ────────────────────────────────────────────────────────────

/// The baseline-relative estimate: what's the advantage of attending to this
/// candidate over what autopilot would have done from this state?
#[derive(Debug, Clone)]
pub struct Advantage {
    /// Free-form description of the candidate.
    pub candidate: String,
    /// What would have happened by default — named, not assumed.
    pub baseline: String,
    /// The estimate: why is this better than baseline?
    pub why_better: String,
}

/// The actual decision made and its shape.
#[derive(Debug, Clone)]
pub struct Action {
    pub kind: ActionKind,
    /// The candidate this decision was made about.
    pub candidate: String,
}

#[derive(Debug, Clone)]
pub enum ActionKind {
    /// Acted on — became a filed issue, a spawned task, a state change.
    Acted { where_it_landed: String },
    /// Held — latent, waiting on a named trigger before actioning.
    Held { activation_trigger: String },
    /// Dropped — explicitly discarded, with a reason.
    Dropped { reason: String },
}

/// The retrospective: did the estimate hold?
/// Written at a loop boundary (day/week/month/quarter/year).
#[derive(Debug, Clone)]
pub struct CreditRecord {
    /// The original advantage estimate, carried forward for comparison.
    pub original_estimate: String,
    /// What actually happened — honest, not post-hoc rationalized.
    pub actual_outcome: String,
    /// Did attending to this turn out better than baseline would have been?
    pub held: bool,
}

// ── the ladder ───────────────────────────────────────────────────────────────

ladder!(AttentionLedger {
    carry {
        candidate_id: String,
    }

    // A candidate has been compared against baseline — Advantage token created.
    Surfaced(Advantage)
        // A decision was made: act, hold, or drop. The Surfaced token is consumed.
        => Decided(Action)
        // The retrospective: did the estimate hold? The Decided token is consumed.
        // THIS IS THE ENFORCED STEP. No path to a terminal state without it.
        => Assessed(CreditRecord)
        => {
            // The estimate held — attending to this was genuinely better than default.
            EstimateHeld
            // The estimate missed — named honestly, not glossed over.
            // Both are terminal: either way, the loop closes with a verdict.
            | EstimateMissed
        }
});

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_types_exist() {
    let _carry = attentionledger::Carry {
        candidate_id: "outer-loop/bets/active/witt3rd/rung/attention-ledger.md".into(),
    };
}

#[test]
fn test_advantage_payload() {
    let adv = Advantage {
        candidate: "build AttentionLedger ladder".into(),
        baseline: "autopilot: keep elaborating on theory docs".into(),
        why_better: "closes the credit-assignment gap structurally; cheap; closes a stale loop".into(),
    };
    // A Surfaced token can only be created by the transition function — not by
    // directly constructing the struct. This is the sealed constructor guarantee.
    // We can hold the payload type without being able to fabricate a rung.
    let _ = adv;
}

#[test]
fn test_decided_payload() {
    let action = Action {
        candidate: "build AttentionLedger ladder".into(),
        kind: ActionKind::Acted {
            where_it_landed: "rung/tests/attention_ledger.rs".into(),
        },
    };
    let _ = action;
}

#[test]
fn test_credit_record_payload() {
    let record = CreditRecord {
        original_estimate: "closes credit-assignment gap; cheap; high advantage vs baseline".into(),
        actual_outcome: "ladder built in one session, tests pass, closes the stated gap".into(),
        held: true,
    };
    let _ = record;
}

#[test]
fn test_verdict_enum_exists() {
    // Both terminal verdicts must exist. The exhaustive match enforces
    // that any code consuming StepOutcome handles both.
    let _outcome: Option<attentionledger::StepOutcome> = None;
}

// ── what the compiler refuses ─────────────────────────────────────────────────
//
// These would be compile errors if uncommented:
//
// fn skip_assessed() {
//     // Cannot get to EstimateHeld without going through Assessed.
//     // Assessed requires a Decided token. Decided requires a Surfaced token.
//     // There is no constructor for any of these types outside the ladder module.
//     let _held = attentionledger::EstimateHeld;  // ✗ unit struct, but...
//     // ...there is no transition that produces EstimateHeld except via step(),
//     // which requires an Assessed token, which requires a Decided token, etc.
//     // The full chain must be walked. Skipping is not expressible.
// }
//
// fn fabricate_assessed() {
//     // Cannot construct Assessed { _seal: (), ... } — _seal is private.
//     let _fake = attentionledger::Assessed { _seal: (), ... }; // ✗ compile error
// }
