//! Inject: what the next turn header carries beyond the facts — one recall
//! block (and its cue), the expectations-due digest, the calendar digest.
//! Everything Inject adds goes into the newest header at the tail, so
//! injecting never breaks the cached prefix.

use std::collections::BTreeMap;

use rung_std::decide::{Decided, Question};
use serde::Serialize;

use super::{HostQuestion, Knobs, choice, noul};
use crate::clock::{HOUR, Millis};
use crate::kernel::TurnKind;
use crate::state::State;

#[derive(Debug, Clone, Serialize)]
pub struct InjectInput {
    /// Gists of what waits (or the commitment's goal and next step).
    pub focus: Vec<String>,
    pub free_session_turns: u64,
    pub turns_since_recall: Option<u64>,
    pub recall_hits_last: u64,
    /// The memory provider, or `off`.
    pub memory: String,
    pub expectations_due_1h: usize,
    pub calendar_within_2h: usize,
    pub note_age_turns: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct InjectCtx {
    pub kind: TurnKind,
    /// The first turn of a commitment.
    pub first_committed: bool,
    pub turn: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cue {
    Stimulus,
    Commitment,
    Note,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InjectChoice {
    pub recall: bool,
    pub cue: Cue,
    pub expectations: bool,
    pub calendar: bool,
}

pub fn build(st: &State, now: Millis, memory: &str) -> InjectInput {
    let mut focus: Vec<String> = st
        .inbox
        .waiting()
        .iter()
        .take(4)
        .map(|p| p.item.gist())
        .collect();
    if let Some(c) = st.kernel.commitment() {
        focus.push(crate::inbox::gist(&format!(
            "commitment: {} — next: {}",
            c.title,
            c.next_step.as_deref().unwrap_or("(none yet)")
        )));
    }
    InjectInput {
        focus,
        free_session_turns: st.kernel.free_session_turns,
        turns_since_recall: st.desk.last_recall_turn.map(|t| st.turn.saturating_sub(t)),
        recall_hits_last: st.desk.recall_hits_last,
        memory: memory.into(),
        expectations_due_1h: st.registers.expectations_due(now, HOUR),
        calendar_within_2h: st.calendar.within(now, 2 * HOUR),
        note_age_turns: st
            .registers
            .note
            .as_ref()
            .map(|(t, _)| st.turn.saturating_sub(*t)),
    }
}

pub struct Inject;

impl HostQuestion for Inject {
    const ID: &'static str = "inject";
    type Input = InjectInput;
    type Ctx = InjectCtx;
    type Choice = InjectChoice;

    fn questions(_input: &InjectInput) -> BTreeMap<String, Question> {
        let mut q = BTreeMap::new();
        q.insert(
            "recall_useful".into(),
            Question::noul(
                "Would one block of recalled long-term memory help the turn about to run?",
                "Yes: something remembered bears on what the turn is about.",
                "No: the turn needs nothing from memory.",
            ),
        );
        q.insert(
            "recall_cue".into(),
            Question::choice(
                "If memory is recalled, what should the recall be about?",
                &[
                    ("stimulus", "The items waiting to be shown."),
                    ("commitment", "The current commitment."),
                    ("note", "The agent's carried note."),
                    ("none", "Nothing: do not recall."),
                ],
            ),
        );
        q.insert(
            "include_expectations".into(),
            Question::noul(
                "Should the next turn header list the expectations coming due?",
                "Yes: one is due soon enough to matter to this turn.",
                "No: none bears on this turn.",
            ),
        );
        q.insert(
            "include_calendar".into(),
            Question::noul(
                "Should the next turn header list the calendar items ahead?",
                "Yes: one is close enough to matter to this turn.",
                "No: none bears on this turn.",
            ),
        );
        q
    }

    fn compose(
        _input: &InjectInput,
        d: &Decided,
        k: &Knobs,
        _ctx: &InjectCtx,
    ) -> Option<InjectChoice> {
        let cue = match choice(d, "recall_cue")? {
            "stimulus" => Cue::Stimulus,
            "commitment" => Cue::Commitment,
            "note" => Cue::Note,
            _ => Cue::None,
        };
        let recall = noul(d, "recall_useful")? >= k.recall_p && cue != Cue::None;
        Some(InjectChoice {
            recall,
            cue: if recall { cue } else { Cue::None },
            expectations: noul(d, "include_expectations")? >= k.digest_p,
            calendar: noul(d, "include_calendar")? >= k.digest_p,
        })
    }

    fn rule(input: &InjectInput, k: &Knobs, ctx: &InjectCtx) -> InjectChoice {
        let cue = match ctx.kind {
            TurnKind::Responding => Cue::Stimulus,
            TurnKind::Committed if ctx.first_committed => Cue::Commitment,
            TurnKind::Free if ctx.turn.is_multiple_of(k.recall_every_free) => Cue::Note,
            _ => Cue::None,
        };
        InjectChoice {
            recall: cue != Cue::None,
            cue,
            expectations: input.expectations_due_1h > 0,
            calendar: input.calendar_within_2h > 0,
        }
    }

    fn guard(
        input: &InjectInput,
        mut c: InjectChoice,
        _k: &Knobs,
        _ctx: &InjectCtx,
    ) -> InjectChoice {
        if input.memory == "off" || c.cue == Cue::None {
            c.recall = false;
            c.cue = Cue::None;
        }
        c
    }
}
