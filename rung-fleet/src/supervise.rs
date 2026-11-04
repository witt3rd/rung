//! Stalls (`a-stalled-crew-is-escalated-not-looped`).
//!
//! A [`Watch`] says, against a clock the caller owns, whether a crew member has
//! been silent past its bound. Silence moves a task to *stalled* only through
//! the ladder (`Signal::Stall`, then `step`), never by convention. A stalled
//! task is handled by [`handle_stall`], which consumes the `Stalled` verdict:
//! the first stall is retried once, and the same task stalling again is
//! **escalated one level**, as an escalation to the steward's parent, instead
//! of retried a third time.

use crate::delivery::{CorrelationId, Delivery, DeliveryError, Kind};
use crate::lifecycle::{Signal, Work, delegation};
use crate::tree::Line;

/// How many times a stalled task is retried before it is escalated.
pub const RETRIES: u32 = 1;

/// Silence tracking against a caller-supplied clock (milliseconds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watch {
    bound: i64,
    last: i64,
}

impl Watch {
    /// Start watching at `now`; silence of more than `bound` is a stall.
    pub fn new(bound: i64, now: i64) -> Self {
        Watch { bound, last: now }
    }

    /// The member did something at `now`.
    pub fn beat(&mut self, now: i64) {
        self.last = now;
    }

    /// Whether the member has been silent past its bound at `now`.
    pub fn silent(&self, now: i64) -> bool {
        now - self.last > self.bound
    }
}

/// Advance an active task by one step, reporting a stall if `watch` says the
/// member is silent at `now`. The one door by which silence becomes *stalled*.
#[allow(clippy::result_large_err)] // the ladder's own failure type, returned as the ladder returns it
pub fn tick(
    mut active: delegation::Active,
    watch: &Watch,
    now: i64,
) -> Result<delegation::StepOutcome, delegation::Failed<delegation::Active>> {
    if watch.silent(now) {
        active.payload.signal = Signal::Stall;
    }
    delegation::step(active)
}

/// A stall that goes up one level.
#[derive(Debug)]
pub struct Escalation {
    /// Who it is for: the steward's parent.
    pub to: String,
    /// The delivery to put in that member's inbox.
    pub delivery: Delivery,
    /// The task as it stood when it was given up on.
    pub work: Work,
}

/// What became of a stalled task.
#[must_use = "a stalled task is retried or escalated, not dropped"]
pub enum Handled {
    /// Retried: the task is active again.
    Retried(delegation::Active),
    /// Given up on, and passed up.
    Escalated(Escalation),
}

/// Why a stall could not be handled.
#[derive(Debug)]
pub enum StallError {
    /// The steward has no parent to escalate to (a chief answers to the
    /// principal, which is not a fleet member).
    NoParent(String),
    /// The escalation's correlation id is not a safe name.
    Delivery(DeliveryError),
}

impl std::fmt::Display for StallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StallError::NoParent(s) => write!(f, "{s} has no parent to escalate to"),
            StallError::Delivery(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StallError {}

/// Handle a stalled task of `steward`'s crew: retry it while it has stalled no
/// more than [`RETRIES`] times, otherwise escalate to the steward's parent.
/// The `Stalled` verdict is consumed either way. The escalation is the same
/// delivery for the same task and stall count, so repeating it adds nothing.
pub fn handle_stall(
    stalled: delegation::Stalled,
    steward: &str,
    line: &Line,
) -> Result<Handled, StallError> {
    let task = stalled.source().carry().task.clone();
    if stalled.source().payload.stalls <= RETRIES {
        return Ok(Handled::Retried(delegation::restart(stalled)));
    }
    let to = line
        .parent_of(steward)
        .ok_or_else(|| StallError::NoParent(steward.to_string()))?
        .to_string();
    let work = stalled.into_source().payload;
    let delivery = Delivery {
        correlation: CorrelationId::new(format!("escalate-{task}-{}", work.stalls))
            .map_err(StallError::Delivery)?,
        kind: Kind::Escalation,
        from: steward.to_string(),
        text: format!(
            "task {task} ({}) stalled {} times; it was retried {RETRIES} time and is not retried again",
            work.assignee, work.stalls
        ),
    };
    Ok(Handled::Escalated(Escalation { to, delivery, work }))
}
