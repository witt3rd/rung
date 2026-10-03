//! Tools: which tool groups are callable in the next turn.
//!
//! The tool definitions never change (the stable superset); this family
//! only moves the host-side call gate. `core` is always on; nothing outside
//! the operator ceiling is ever on; a switched group holds for a few turns
//! (hysteresis) so the gate does not flap.

use std::collections::{BTreeMap, BTreeSet};

use rung_std::decide::{Decided, Question};
use serde::Serialize;

use super::{HostQuestion, Knobs, noul, qid};
use crate::kernel::TurnKind;
use crate::state::State;
use crate::toolbox::{CORE, MEMORY, READ, WORKSPACE_WRITE};

#[derive(Debug, Clone, Serialize)]
pub struct ToolsInput {
    pub kind: TurnKind,
    pub enabled: Vec<String>,
    pub ceiling: Vec<String>,
    pub wants: Vec<(String, String)>,
    pub uses_last_10: BTreeMap<String, u32>,
    pub turns_since_switch: BTreeMap<String, u64>,
}

#[derive(Debug, Clone)]
pub struct ToolsCtx {
    pub kind: TurnKind,
    pub turn: u64,
    /// Groups with a `want_tools` request within the rule's window.
    pub wanted_recently: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolsChoice {
    /// Sorted.
    pub enabled: Vec<String>,
}

pub fn build(st: &State, ceiling: &BTreeSet<String>, kind: TurnKind, k: &Knobs) -> (ToolsInput, ToolsCtx) {
    let turn = st.turn + 1;
    let mut uses = BTreeMap::new();
    for m in &st.desk.uses {
        for (g, n) in m {
            *uses.entry(g.clone()).or_default() += n;
        }
    }
    let input = ToolsInput {
        kind,
        enabled: st
            .desk
            .groups
            .iter()
            .filter(|(_, s)| s.enabled)
            .map(|(g, _)| g.clone())
            .collect(),
        ceiling: ceiling.iter().cloned().collect(),
        wants: st
            .desk
            .wants
            .iter()
            .filter(|w| w.turn + k.want_turns >= turn)
            .map(|w| (w.group.clone(), crate::inbox::gist(&w.why)))
            .collect(),
        uses_last_10: uses,
        turns_since_switch: st
            .desk
            .groups
            .iter()
            .map(|(g, s)| (g.clone(), turn.saturating_sub(s.since_turn)))
            .collect(),
    };
    let ctx = ToolsCtx {
        kind,
        turn,
        wanted_recently: st
            .desk
            .wants
            .iter()
            .filter(|w| w.turn + k.want_turns >= turn)
            .map(|w| w.group.clone())
            .collect(),
    };
    (input, ctx)
}

pub struct Tools;

fn askable(input: &ToolsInput) -> impl Iterator<Item = &String> {
    input.ceiling.iter().filter(|g| g.as_str() != CORE)
}

impl HostQuestion for Tools {
    const ID: &'static str = "tools";
    type Input = ToolsInput;
    type Ctx = ToolsCtx;
    type Choice = ToolsChoice;

    fn questions(input: &ToolsInput) -> BTreeMap<String, Question> {
        askable(input)
            .map(|g| {
                (
                    qid("enable", g),
                    Question::noul(
                        &format!("Should the tool group `{g}` be callable in the turn about to run?"),
                        "Yes: the turn plausibly needs it, or the agent asked for it with a reason.",
                        "No: the turn does not need it.",
                    ),
                )
            })
            .collect()
    }

    fn compose(input: &ToolsInput, d: &Decided, k: &Knobs, _ctx: &ToolsCtx) -> Option<ToolsChoice> {
        let mut on = vec![CORE.to_string()];
        for g in askable(input) {
            let p = noul(d, &qid("enable", g))?;
            let was = input.enabled.contains(g);
            let since = input.turns_since_switch.get(g).copied().unwrap_or(u64::MAX);
            let want = if was && since < k.stay_on_turns {
                true
            } else if !was && since < k.stay_off_turns {
                false
            } else {
                p >= k.enable_p
            };
            if want {
                on.push(g.clone());
            }
        }
        Some(ToolsChoice { enabled: on })
    }

    fn rule(input: &ToolsInput, _k: &Knobs, ctx: &ToolsCtx) -> ToolsChoice {
        let mut on = vec![CORE.to_string()];
        for g in askable(input) {
            let want = match g.as_str() {
                MEMORY | READ => true,
                WORKSPACE_WRITE => ctx.kind != TurnKind::Free,
                _ => ctx.wanted_recently.contains(g),
            };
            if want {
                on.push(g.clone());
            }
        }
        ToolsChoice { enabled: on }
    }

    fn guard(input: &ToolsInput, c: ToolsChoice, _k: &Knobs, _ctx: &ToolsCtx) -> ToolsChoice {
        let mut on: BTreeSet<String> = c
            .enabled
            .into_iter()
            .filter(|g| input.ceiling.contains(g))
            .collect();
        on.insert(CORE.to_string());
        ToolsChoice {
            enabled: on.into_iter().collect(),
        }
    }
}
