//! Consolidate: before anything is evicted, should the agent be offered a
//! note update, and which host observations go to long-term memory?
//! Asked with every Pack ask, and every 20 turns otherwise. What is
//! retained is only ever the agent's own text or the host's mechanical
//! observations; raw tool output never is.

use std::collections::BTreeMap;

use rung_std::decide::{Decided, Question};
use serde::Serialize;

use super::{Candidate, HostQuestion, Knobs, noul, qid};
use crate::state::State;

#[derive(Debug, Clone, Serialize)]
pub struct CandidateView {
    pub id: String,
    pub kind: String,
    pub gist: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConsolidateInput {
    pub note_age_turns: Option<u64>,
    pub commits: u64,
    pub releases: u64,
    pub candidates: Vec<CandidateView>,
    /// A rollover is close (the pack is past the rule's break threshold or
    /// a copy loop is flagged).
    pub rollover_imminent: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConsolidateChoice {
    pub note_line: bool,
    pub retain: Vec<String>,
    /// Every candidate this decision covered (retained or not).
    pub considered: Vec<String>,
}

pub fn build(st: &State, k: &Knobs, rollover_imminent: bool) -> (ConsolidateInput, Vec<Candidate>) {
    let cands: Vec<Candidate> = st
        .desk
        .candidates
        .iter()
        .take(k.max_candidates)
        .cloned()
        .collect();
    let input = ConsolidateInput {
        note_age_turns: st.registers.note.as_ref().map(|(t, _)| st.turn.saturating_sub(*t)),
        commits: st.kernel.commits,
        releases: st.kernel.releases,
        candidates: cands
            .iter()
            .map(|c| CandidateView {
                id: c.id.clone(),
                kind: c.kind.clone(),
                gist: c.gist.clone(),
            })
            .collect(),
        rollover_imminent,
    };
    (input, cands)
}

pub struct Consolidate;

fn considered(input: &ConsolidateInput) -> Vec<String> {
    input.candidates.iter().map(|c| c.id.clone()).collect()
}

impl HostQuestion for Consolidate {
    const ID: &'static str = "consolidate";
    type Input = ConsolidateInput;
    type Ctx = ();
    type Choice = ConsolidateChoice;

    fn questions(input: &ConsolidateInput) -> BTreeMap<String, Question> {
        let mut q = BTreeMap::new();
        q.insert(
            "note_due".into(),
            Question::noul(
                "Should the agent be asked to update its carried note before anything is evicted?",
                "Yes: enough has happened since its note that it may want to carry something.",
                "No: its note is recent enough.",
            ),
        );
        for c in &input.candidates {
            q.insert(
                qid("retain", &c.id),
                Question::noul(
                    &format!("Is observation {} worth keeping in long-term memory?", c.id),
                    "Yes: it records something worth remembering later.",
                    "No: it is routine.",
                ),
            );
        }
        q
    }

    fn compose(input: &ConsolidateInput, d: &Decided, k: &Knobs, _ctx: &()) -> Option<ConsolidateChoice> {
        let old = input.note_age_turns.is_none_or(|a| a > k.note_age_turns);
        let note_line = noul(d, "note_due")? >= k.note_p || (input.rollover_imminent && old);
        let mut retain = Vec::new();
        for c in &input.candidates {
            if noul(d, &qid("retain", &c.id))? >= k.retain_p {
                retain.push(c.id.clone());
            }
        }
        Some(ConsolidateChoice {
            note_line,
            retain,
            considered: considered(input),
        })
    }

    fn rule(input: &ConsolidateInput, _k: &Knobs, _ctx: &()) -> ConsolidateChoice {
        ConsolidateChoice {
            note_line: true,
            retain: considered(input),
            considered: considered(input),
        }
    }

    fn guard(input: &ConsolidateInput, mut c: ConsolidateChoice, _k: &Knobs, _ctx: &()) -> ConsolidateChoice {
        let ids = considered(input);
        c.retain.retain(|id| ids.contains(id));
        c.considered = ids;
        c
    }
}
