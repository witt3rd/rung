//! Admit: is a waiting item worth interrupting the current mode for?
//!
//! Each waiting item is shown now (displacing the planned continuation),
//! held until the agent's next break, or shown as one digest line in the
//! next turn header (no displacement). Owner items never reach the
//! decider: they are `now` at the next boundary, always.

use std::collections::BTreeMap;

use rung_std::decide::{Decided, Question};
use serde::Serialize;
use serde_json::{Value, json};

use super::{HostQuestion, Knobs, choice, noul, qid};
use crate::clock::Millis;
use crate::inbox::{ItemKind, Role};
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    Now,
    AtBreak,
    Digest,
}

/// An item as the decider sees it.
#[derive(Debug, Clone, Serialize)]
pub struct AdmitItem {
    pub id: String,
    pub kind: ItemKind,
    pub role: Role,
    pub age_s: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_in_s: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_urgency: Option<String>,
    pub gist: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdmitInput {
    pub now: Millis,
    pub mode: Value,
    /// The asked items: non-owner, oldest first, at most `max_items`.
    pub items: Vec<AdmitItem>,
    pub interrupts_last_hour: u32,
    pub since_external_s: i64,
}

/// A waiting item as the guards see it.
#[derive(Debug, Clone)]
pub struct Waiting {
    pub id: String,
    pub kind: ItemKind,
    pub role: Role,
    pub at: Millis,
    pub due: Option<Millis>,
    pub firm: bool,
}

#[derive(Debug, Clone)]
pub struct AdmitCtx {
    pub now: Millis,
    /// Every waiting item, oldest first (owner items included).
    pub waiting: Vec<Waiting>,
    pub at_break: bool,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AdmitChoice {
    pub forms: BTreeMap<String, Form>,
    /// 1 when this boundary interrupts a commitment.
    pub interrupts: u32,
}

impl AdmitChoice {
    pub fn now(&self) -> Vec<String> {
        self.with(Form::Now)
    }
    pub fn digests(&self) -> Vec<String> {
        self.with(Form::Digest)
    }
    fn with(&self, f: Form) -> Vec<String> {
        self.forms
            .iter()
            .filter(|(_, x)| **x == f)
            .map(|(id, _)| id.clone())
            .collect()
    }
}

fn kind_key(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Peer => "peer",
        ItemKind::Calendar => "calendar",
        ItemKind::Expectation => "expectation",
        ItemKind::World => "world",
        ItemKind::Memory => "memory",
        ItemKind::Completion => "completion",
    }
}

/// The input and context from the host's state at `now`.
pub fn build(st: &State, now: Millis, k: &Knobs) -> (AdmitInput, AdmitCtx) {
    let waiting: Vec<Waiting> = st
        .inbox
        .waiting()
        .into_iter()
        .map(|p| Waiting {
            id: p.item.id.clone(),
            kind: p.item.kind,
            role: p.item.role,
            at: p.item.at,
            due: p.item.due,
            firm: p.item.firm,
        })
        .collect();
    let items = st
        .inbox
        .waiting()
        .into_iter()
        .filter(|p| p.item.role != Role::Owner)
        .take(k.max_items)
        .map(|p| AdmitItem {
            id: p.item.id.clone(),
            kind: p.item.kind,
            role: p.item.role,
            age_s: (now - p.item.at) / 1000,
            due_in_s: p.item.due.map(|d| (d - now) / 1000),
            declared_urgency: p.item.urgency.clone(),
            gist: p.item.gist(),
        })
        .collect();
    let mode = match st.kernel.commitment() {
        Some(c) => json!({"committed": {
            "project": c.project, "title": crate::inbox::gist(&c.title),
            "since_turns": st.turn.saturating_sub(c.since_turn),
            "done_when": crate::inbox::gist(&c.done_when),
        }}),
        None => json!({"free": {"session_turns": st.kernel.free_session_turns}}),
    };
    let input = AdmitInput {
        now,
        mode,
        items,
        interrupts_last_hour: st.desk.interrupts.len() as u32,
        since_external_s: st
            .inbox
            .last_external_at
            .map(|t| (now - t) / 1000)
            .unwrap_or(-1),
    };
    let ctx = AdmitCtx {
        now,
        waiting,
        at_break: st.kernel.at_break(),
        committed: st.kernel.commitment().is_some(),
    };
    (input, ctx)
}

pub struct Admit;

fn rule_form(w: &Waiting, now: Millis) -> Form {
    match (w.role, w.kind) {
        (Role::Owner, _) => Form::Now,
        (_, ItemKind::Calendar) if w.due.is_none_or(|d| d <= now) => Form::Now,
        (_, ItemKind::Expectation) => Form::Digest,
        _ => Form::AtBreak,
    }
}

impl HostQuestion for Admit {
    const ID: &'static str = "admit";
    type Input = AdmitInput;
    type Ctx = AdmitCtx;
    type Choice = AdmitChoice;

    fn questions(input: &AdmitInput) -> BTreeMap<String, Question> {
        let mut q = BTreeMap::new();
        for it in &input.items {
            q.insert(
                qid("interrupt", &it.id),
                Question::noul(
                    &format!(
                        "Should waiting item {} be shown to the agent at this boundary, displacing the planned continuation, rather than wait for the agent's next break? Judge urgency and relevance from the state only.",
                        it.id
                    ),
                    "Show it now: waiting would cost the agent or the sender something real.",
                    "It can wait for the next break without real cost.",
                ),
            );
            q.insert(
                qid("form", &it.id),
                Question::choice(
                    &format!("How should waiting item {} reach the agent?", it.id),
                    &[
                        ("now", "Shown in full at this boundary."),
                        ("at_break", "Held until the agent's next natural break."),
                        ("digest", "One line in the next turn header, without displacing anything."),
                    ],
                ),
            );
        }
        q
    }

    fn compose(input: &AdmitInput, d: &Decided, k: &Knobs, ctx: &AdmitCtx) -> Option<AdmitChoice> {
        let mut forms = BTreeMap::new();
        for w in &ctx.waiting {
            forms.insert(w.id.clone(), rule_form(w, ctx.now));
        }
        for it in &input.items {
            let p = noul(d, &qid("interrupt", &it.id))?;
            let form = choice(d, &qid("form", &it.id))?;
            let f = if p >= k.admit_now_p || form == "now" {
                Form::Now
            } else if form == "digest" {
                Form::Digest
            } else {
                Form::AtBreak
            };
            forms.insert(it.id.clone(), f);
        }
        Some(AdmitChoice {
            forms,
            interrupts: 0,
        })
    }

    fn rule(_input: &AdmitInput, _k: &Knobs, ctx: &AdmitCtx) -> AdmitChoice {
        AdmitChoice {
            forms: ctx
                .waiting
                .iter()
                .map(|w| (w.id.clone(), rule_form(w, ctx.now)))
                .collect(),
            interrupts: 0,
        }
    }

    fn guard(input: &AdmitInput, mut c: AdmitChoice, k: &Knobs, ctx: &AdmitCtx) -> AdmitChoice {
        let mut forced = Vec::new();
        for w in &ctx.waiting {
            let max = k.max_deferral.get(kind_key(w.kind)).copied().unwrap_or(Millis::MAX);
            let must = w.role == Role::Owner
                || (w.firm && w.due.is_none_or(|d| d <= ctx.now))
                || ctx.now - w.at >= max;
            let f = c.forms.entry(w.id.clone()).or_insert(Form::AtBreak);
            if must {
                *f = Form::Now;
                forced.push(w.id.clone());
            } else if *f == Form::AtBreak && ctx.at_break {
                *f = Form::Now;
            }
        }
        // Only waiting items carry a form.
        c.forms.retain(|id, _| ctx.waiting.iter().any(|w| &w.id == id));
        let owner_now = ctx
            .waiting
            .iter()
            .any(|w| w.role == Role::Owner && c.forms.get(&w.id) == Some(&Form::Now));
        if ctx.committed && !owner_now && input.interrupts_last_hour >= k.max_interrupts_per_hour
        {
            for (id, f) in c.forms.iter_mut() {
                if *f == Form::Now && !forced.contains(id) {
                    *f = Form::AtBreak;
                }
            }
        }
        let any_now = c.forms.values().any(|f| *f == Form::Now);
        c.interrupts = u32::from(ctx.committed && any_now);
        c
    }
}
