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
    /// The epoch's mechanical outline so far, newest last (bounded).
    pub outline: Vec<String>,
    /// The previous epoch's outline.
    pub prev_outline: Vec<String>,
    /// Superset swaps so far.
    pub swaps: u64,
}

/// Outline lines kept per epoch.
pub const OUTLINE_LINES: usize = 24;

impl PackMeta {
    fn note(&mut self, s: String) {
        self.outline.push(s.chars().take(140).collect());
        if self.outline.len() > OUTLINE_LINES {
            self.outline.remove(0);
        }
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
            "kernel.commit" => self.note(format!("committed to {} ({})", l.str("project"), l.str("title"))),
            "kernel.release" => self.note(format!(
                "released {} ({}, by {})",
                l.str("project"),
                l.str("outcome"),
                l.str("released_by")
            )),
            "expectation.settled" => self.note(format!("expectation {} {}", l.str("id"), l.str("state"))),
            "model.switch" => self.note(format!("model {} → {} ({})", l.str("from"), l.str("to"), l.str("why"))),
            _ => {}
        }
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
}

impl State {
    pub fn apply(&mut self, l: &Line) {
        self.last_at = l.at;
        match l.kind.as_str() {
            "turn.started" => self.turn = l.u64("turn"),
            "boundary" => self.boundary = l.u64("n"),
            _ => {}
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

    /// The canonical hash of the whole state.
    pub fn hash(&self) -> String {
        canon::hash(&canon::of(self))
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
