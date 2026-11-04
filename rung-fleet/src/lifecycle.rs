//! The delegation lifecycle as typed ladders (props section 3).
//!
//! [`delegation`] runs *spawned, assigned, active* and ends in *done*, with
//! *held* and *stalled* as recoverable verdicts of *active*. [`closure`] takes
//! a task from *done* to *retired*. A skipped transition is a compile error
//! (`a-skipped-transition-does-not-compile`).
//!
//! The ladder's own functions take only the token, so the one input a step
//! reads is the `signal` on the [`delegation::Active`] payload; the helpers
//! here are the typed doors that set it.

use crate::brief::{Brief, BriefError};
use crate::landing::Landed;
use rung::ladder;

/// The key a hold carries. Only a resolution with the same key closes it
/// (`a-decision-is-a-held-item`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoldKey(pub String);

/// What a finished task reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub summary: String,
}

/// A spawned task: what it is (its brief) and who is to do it. A task cannot
/// be spawned without a brief (`a-brief-is-a-typed-contract`).
#[derive(Clone, Debug)]
pub struct Spawn {
    brief: Brief,
    assignee: String,
}

impl Spawn {
    /// Spawn a task for `assignee`. The brief is already validated; an empty
    /// assignee is refused.
    pub fn new(brief: Brief, assignee: impl Into<String>) -> Result<Self, BriefError> {
        let assignee = assignee.into();
        if assignee.trim().is_empty() {
            return Err(BriefError::Empty("assignee"));
        }
        Ok(Spawn { brief, assignee })
    }

    pub fn brief(&self) -> &Brief {
        &self.brief
    }

    pub fn assignee(&self) -> &str {
        &self.assignee
    }
}

/// A task assigned to a worker with the brief it was given.
#[derive(Clone, Debug)]
pub struct Assignment {
    brief: Brief,
    assignee: String,
}

impl Assignment {
    pub fn brief(&self) -> &Brief {
        &self.brief
    }

    pub fn assignee(&self) -> &str {
        &self.assignee
    }
}

/// What the worker reported since the last step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Nothing new: a beat of progress.
    Beat,
    /// Waiting for a decision only the principal can make.
    Hold(HoldKey),
    /// Silent past its bound.
    Stall,
    /// The work is complete.
    Finish(Completion),
}

/// The working state of an active task. `Clone + PartialEq` is what the
/// progress guard on a recoverable verdict compares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Work {
    pub brief: Brief,
    pub assignee: String,
    pub beats: u32,
    pub stalls: u32,
    /// The hold the task waits on, if held.
    pub hold: Option<HoldKey>,
    /// The resolution that answers `hold`, once one has arrived.
    pub resolution: Option<HoldKey>,
    /// The input the next step reads.
    pub signal: Signal,
}

ladder!(Delegation {
    carry { task: String }

    Spawned(Spawn)
      => Assigned(Assignment)
      => Active(Work)
      => {
          Progress -> Active
          | Held => Active
          | Stalled => Active
          | Done(Completion)
      }

    recover {
        resume: Held => Active
        restart: Stalled => Active
    }
} impl {
    assigned = |spawned| {
        let carry = spawned.carry().clone();
        Assigned::new(
            Assignment { brief: spawned.payload.brief, assignee: spawned.payload.assignee },
            carry,
        )
    },
    active = |assigned| {
        let carry = assigned.carry().clone();
        Active::new(
            Work {
                brief: assigned.payload.brief,
                assignee: assigned.payload.assignee,
                beats: 0,
                stalls: 0,
                hold: None,
                resolution: None,
                signal: Signal::Beat,
            },
            carry,
        )
    },
    step = |active| {
        let carry = active.carry().clone();
        let mut work = active.payload.clone();
        // A hold stays open until a resolution carrying its key arrives:
        // nothing else moves a held task (`held-and-stalled-are-recoverable`).
        if work.hold.is_some() {
            work.signal = Signal::Beat;
            return Ok(StepOutcome::Held(Held::new(Active::new(work, carry))));
        }
        match std::mem::replace(&mut work.signal, Signal::Beat) {
            Signal::Beat => {
                work.beats += 1;
                Ok(StepOutcome::Progress(Active::new(work, carry)))
            }
            Signal::Hold(key) => {
                work.hold = Some(key);
                Ok(StepOutcome::Held(Held::new(Active::new(work, carry))))
            }
            Signal::Stall => {
                work.stalls += 1;
                Ok(StepOutcome::Stalled(Stalled::new(Active::new(work, carry))))
            }
            Signal::Finish(c) => Ok(StepOutcome::Done(Done::new(c))),
        }
    },
    resume = |held| {
        let active = held.into_source();
        let carry = active.carry().clone();
        let mut work = active.payload;
        if work.resolution.is_some() && work.resolution == work.hold {
            work.hold = None;
            work.resolution = None;
        }
        // A resolution for another key leaves the hold open; the next step
        // holds again.
        work.beats += 1;
        Active::new(work, carry)
    },
    restart = |stalled| {
        let active = stalled.into_source();
        let carry = active.carry().clone();
        let mut work = active.payload;
        work.beats += 1;
        Active::new(work, carry)
    },
});

impl Work {
    /// Record a resolution. It closes the hold only if its key is the hold's.
    pub fn resolve(&mut self, key: HoldKey) {
        self.resolution = Some(key);
    }
}

/// Proof that a task reached *done*: constructible only from the
/// [`delegation::Done`] token, so the retirement ladder cannot be entered
/// without going through the delegation ladder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finished {
    completion: Completion,
    landed: Landed,
}

impl Finished {
    /// The only way to hold a `Finished`: consume the `Done` verdict and the
    /// proof its work has landed (`unlanded-work-is-never-retired`).
    pub fn from_done(done: delegation::Done, landed: Landed) -> Self {
        Finished {
            completion: done.into_payload(),
            landed,
        }
    }

    pub fn completion(&self) -> &Completion {
        &self.completion
    }
}

/// What a retired task leaves behind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Retirement {
    pub completion: Completion,
    /// How its work landed, in words for the record.
    pub landed: String,
}

ladder!(Closure {
    carry { task: String }

    Finishing(Finished) => { Retired(Retirement) }
} impl {
    step = |finishing| {
        Ok(StepOutcome::Retired(Retired::new(Retirement {
            completion: finishing.payload.completion().clone(),
            landed: finishing.payload.landed.describe(),
        })))
    },
});
