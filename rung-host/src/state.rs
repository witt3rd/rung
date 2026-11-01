//! The host's state: every register, as the record leaves it.
//!
//! [`State::apply`] is the only way state changes, and it is pure: the same
//! lines in the same order give the same state, byte for byte
//! ([`State::hash`]). The live host applies each line as it writes it; a
//! waking host replays the record. Gate G-i compares the two.

use serde::Serialize;

use crate::calendar::CalendarState;
use crate::canon;
use crate::clock::Millis;
use crate::desk::DeskState;
use crate::governor::GovState;
use crate::inbox::InboxState;
use crate::kernel::KernelState;
use crate::record::Line;
use crate::registers::Registers;

/// What the pack needs to survive a restart.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PackMeta {
    pub epoch: u64,
    pub epoch_started_at: Millis,
    pub epoch_first_turn: u64,
    /// The epoch's mechanical outline so far, newest last. It restarts at each epoch rollover.
    pub outline: Vec<String>,
    /// The previous epoch's outline.
    pub prev_outline: Vec<String>,
    /// Superset swaps so far.
    pub swaps: u64,
}

impl PackMeta {
    fn note(&mut self, s: String) {
        self.outline.push(s);
    }

    pub fn apply(&mut self, l: &Line) {
        match l.kind.as_str() {
            "epoch.rollover" => {
                self.prev_outline = std::mem::take(&mut self.outline);
                self.epoch = l.u64("to");
                self.epoch_started_at = l.at;
                self.epoch_first_turn = l.u64("first_turn");
            }
            "pack.swap" => self.swaps += 1,
            "turn.ended" => self.note(format!(
                "turn {} {} {}{}",
                l.u64("turn"),
                l.str("turn_kind"),
                l.str("status"),
                match l.get("final_text").as_str() {
                    Some(t) if !t.is_empty() => format!(": {}", crate::inbox::gist(t)),
                    _ => String::new(),
                }
            )),
            "kernel.commit" => self.note(format!(
                "committed to {} ({})",
                l.str("project"),
                l.str("title")
            )),
            "kernel.release" => self.note(format!(
                "released {} ({}, by {})",
                l.str("project"),
                l.str("outcome"),
                l.str("released_by")
            )),
            "expectation.settled" => {
                self.note(format!("expectation {} {}", l.str("id"), l.str("state")))
            }
            "model.switch" => self.note(format!(
                "model {} → {} ({})",
                l.str("from"),
                l.str("to"),
                l.str("why")
            )),
            _ => {}
        }
    }
}

/// What a record line does to whether the host is resting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idle {
    /// An `idle` line: resting until this wake.
    Rests(Millis),
    /// A line that proves the host awake (the rest ended, a boundary began,
    /// the host started or halted).
    Wakes,
    /// Any other line: a resting host writes some without waking (a channel
    /// opened, a calendar entry added).
    Unchanged,
}

/// The one rule for reading rest off the record: the state's fold and the
/// registry's look at the tail of a record both use it.
pub fn idle_effect(l: &Line) -> Idle {
    match l.kind.as_str() {
        "idle" => Idle::Rests(l.i64("until")),
        "idle.ended" | "boundary" | "halted" | "host.start" => Idle::Wakes,
        _ => Idle::Unchanged,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct State {
    /// The last turn started.
    pub turn: u64,
    /// The last boundary.
    pub boundary: u64,
    /// The time of the last line.
    pub last_at: Millis,
    pub inbox: InboxState,
    pub calendar: CalendarState,
    pub registers: Registers,
    pub kernel: KernelState,
    pub desk: DeskState,
    pub governor: GovState,
    pub pack: PackMeta,
    /// The scheduled wake of a resting host: set by an `idle` line, cleared
    /// by the line that ends the rest (and by anything that proves the host
    /// awake). `None` for a host that is not resting. Not part of the hash:
    /// a record written before idle mode folds to the hash it was written with.
    #[serde(skip)]
    pub idle_until: Option<Millis>,
}

impl State {
    pub fn apply(&mut self, l: &Line) {
        self.last_at = l.at;
        match l.kind.as_str() {
            "turn.started" => self.turn = l.u64("turn"),
            "boundary" => self.boundary = l.u64("n"),
            _ => {}
        }
        match idle_effect(l) {
            Idle::Rests(until) => self.idle_until = Some(until),
            Idle::Wakes => self.idle_until = None,
            Idle::Unchanged => {}
        }
        self.inbox.apply(l);
        self.calendar.apply(l);
        self.registers.apply(l);
        self.kernel.apply(l);
        self.desk.apply(l);
        self.governor.apply(l);
        self.pack.apply(l);
    }

    /// Replay `lines` from nothing.
    pub fn replay(lines: &[Line]) -> Self {
        let mut s = Self::default();
        for l in lines {
            s.apply(l);
        }
        s
    }

    /// The hash of the whole state. Every map in it is ordered (BTreeMap,
    /// or a JSON value whose keys the record sorted), so its serialization
    /// is a function of the state.
    pub fn hash(&self) -> String {
        canon::hash(&serde_json::to_vec(self).unwrap_or_default())
    }

    /// For each `turn.ended` line, the hash of the state replayed up to
    /// (not including) it: what gate G-i compares with the line's own
    /// `projection`.
    pub fn replay_hashes(lines: &[Line]) -> std::collections::BTreeMap<u64, String> {
        let mut s = Self::default();
        let mut out = std::collections::BTreeMap::new();
        for l in lines {
            if l.kind == "turn.ended" {
                out.insert(l.seq, s.hash());
            }
            s.apply(l);
        }
        out
    }
}
