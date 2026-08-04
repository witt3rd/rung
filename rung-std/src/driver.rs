//! The driver — hold suspended runs, resume the ones evidence answers.
//!
//! A judgmental dispatch that cannot be settled now hands the argument back
//! unconsumed, with an opaque reference to what was raised (`G16`). Something
//! has to hold that run until the raised matter terminates. This is that
//! something, and its whole design problem is **how little it is allowed to
//! do**.
//!
//! ## The boundary, stated first
//!
//! A driver is mechanism. Every question of the form *which* raised matter to
//! pursue, *how long* to wait, *what order* to resume in, or *whether to give
//! up* is a question about worth, and Het declares no worth law
//! (`het-declares-no-worth-law`, `ordering-is-hetopts`). Those belong to
//! HetOpt. Cross that line and the driver stops being a driver and becomes an
//! unstated policy with a scheduler's name.
//!
//! So this park has, deliberately:
//!
//! - **no ordering.** Not a queue, not a stack, not a priority heap. Evidence
//!   arrives and the runs it answers are released. Which run gets to move is
//!   decided by which question was answered, never by the park.
//! - **no cap on depth.** Nesting is normal — answering one question routinely
//!   raises a second (Q11 raised Q12) — so a park routinely holds many
//!   suspended runs at once. A fixed-size park would evict, and eviction is a
//!   worth judgment about which run matters least.
//! - **no cap on re-entry.** The same run may suspend and resume without bound
//!   (`no-bound-on-reentry`). A park that counted rounds and stopped would be
//!   the eviction rule `guarded-reentry-is-eviction` forbids, relocated.
//! - **no timeout.** A raised matter that never terminates leaves its run
//!   parked forever, and [`awaiting`](Park::awaiting) makes that *visible*
//!   rather than resolving it. Making a block visible is mechanism; deciding a
//!   block has gone on too long is worth.
//!
//! What is left is: put a run down, pick it up when its answer arrives, and say
//! what is still waiting.
//!
//! ## What it does not do
//!
//! **It does not resume.** Resumption writes a rung, which `G2` seals inside
//! the ladder's module, so the resume edge is emitted there and takes an
//! [`Authorized`](rung::Authorized) pen (`resumption-is-authorial`).
//! [`claim`](Park::claim) hands back the [`Suspended`] and the caller invokes
//! the ladder's own resume edge with a pen. A park that could resume would be a
//! door in the seal.
//!
//! **It does not interpret the reference.** Matching is
//! [`Terminated::answers`], the theory's own predicate — the park never
//! compares references itself, never orders them, and never asks whether one is
//! well-formed (`pool-is-opaque`, `raised-reference-is-opaque`).
//!
//! **It does not survive process death.** Everything here is in memory, which
//! is the whole of the claim `suspension-is-in-process-only` makes. Whether a
//! suspended run can be written to durable storage at all is Q13, open:
//! deserializing a mid-ladder rung fabricates a state no arrow in this process
//! reached. There is no `serde` here on purpose, and adding it is not a small
//! change.

use rung::{Awaiting, Raised, Terminated};

/// Suspended runs, held until the matters they await terminate.
///
/// Generic over the suspension type, so one park serves one ladder. A driver
/// coordinating several ladders holds several parks — heterogeneity would need
/// type erasure, and erasing a sealed token to `Any` to put two ladders in one
/// container is a larger claim than this needs to make.
///
/// `S: Awaiting` is what makes the park unable to be lied to: it reads what a
/// run awaits *off the run*, so there is no parameter through which a caller
/// could park a run under a matter it never raised.
#[derive(Debug, Default)]
pub struct Park<S> {
    parked: Vec<S>,
}

impl<S: Awaiting> Park<S> {
    /// An empty park.
    pub fn new() -> Self {
        Self { parked: Vec::new() }
    }

    /// Put a suspended run down.
    ///
    /// Takes ownership, which is what discharges `Suspended`'s `#[must_use]`
    /// honestly: the run is not dropped, it is held. Nothing is authorized by
    /// parking — the gated act is resumption, and it is elsewhere.
    ///
    /// Accepts a run awaiting a matter another parked run also awaits. Two runs
    /// blocked on one question is ordinary, and `claim` releases both.
    pub fn park(&mut self, run: S) {
        self.parked.push(run);
    }

    /// Release **every** run this evidence answers.
    ///
    /// Every, not the first. If two runs await the same matter, the same
    /// terminal answers both, and returning one would leave the park choosing
    /// which — a preference among runs, which is the thing it must not have.
    /// Returning all of them removes the choice rather than making it well.
    ///
    /// Returns empty when the evidence answers nothing parked. That is not an
    /// error: evidence for a matter this park never held is simply not its
    /// business, and a park that failed loudly on it would be asserting a claim
    /// about what references exist — which is the theory's, not Het's.
    ///
    /// Matching is [`Terminated::answers`] and nothing else.
    pub fn claim(&mut self, evidence: &Terminated) -> Vec<S> {
        let mut released = Vec::new();
        let mut i = 0;
        while i < self.parked.len() {
            if evidence.answers(self.parked[i].awaiting()) {
                released.push(self.parked.remove(i));
            } else {
                i += 1;
            }
        }
        released
    }

    /// What every parked run is waiting on.
    ///
    /// The visibility half of composition-note item 7: a raised matter may
    /// never terminate, and the composition should make that block visible
    /// rather than resolve it. This is where a run that has been waiting since
    /// March shows up. It reports; it does not act.
    ///
    /// Order is insertion order, and carries no meaning — it is not a queue
    /// position and nothing should read it as one.
    pub fn awaiting(&self) -> impl Iterator<Item = &Raised> {
        self.parked.iter().map(Awaiting::awaiting)
    }

    /// How many runs are parked.
    pub fn depth(&self) -> usize {
        self.parked.len()
    }

    /// Whether anything is parked.
    pub fn is_empty(&self) -> bool {
        self.parked.is_empty()
    }
}
