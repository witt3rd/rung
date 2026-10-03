//! The registers: the agent's todo, projects, open questions, carried note
//! and expectations, plus the world facts the host has seen. All are
//! projections of the record.
//!
//! The expectation register: the agent states a claim, a warrant, a
//! probability and a due time (`expect`), and may revise the probability
//! (`revise`; the old value stays in the record). It has no way to settle
//! one. The host settles decidable checks from what it has recorded (a
//! world fact, a stimulus from a channel); a disjoint [`Judge`] settles
//! judged ones. A settlement is a [`Settlement`], built only in this module
//! and written through the record's sealed path (gate G-e). Surprise is
//! −log₂ p of the outcome; calibration (Brier, reliability, resolution) is
//! kept as the record goes.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::clock::Millis;
use crate::gates;
use crate::record::Line;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Todo {
    pub text: String,
    pub created_at: Millis,
    pub done: bool,
    /// The agent's own score, if it gave one. Never used to order anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub title: String,
    pub why: String,
    /// `seed`, `active`, `paused`, `done`, `abandoned`.
    pub status: String,
    pub created_at: Millis,
    pub last_step_at: Millis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub text: String,
    pub created_at: Millis,
    pub open: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
}

/// How an expectation is checked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    /// Met when the world asserts `key = equals` before due.
    WorldFact { key: String, equals: Value },
    /// Met when a stimulus arrives from `channel` before due.
    StimulusFrom { channel: String },
    /// A principal disjoint from the agent settles it at due.
    Judged { principal: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpState {
    Open,
    Met,
    Missed,
    Void,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expectation {
    pub claim: String,
    pub about: String,
    pub warrant: String,
    pub p: f64,
    pub made_at: Millis,
    pub turn: u64,
    pub due: Millis,
    pub check: Check,
    pub state: ExpState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surprise: Option<f64>,
}

/// Running calibration over settled expectations.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Calibration {
    pub n: u64,
    pub brier_sum: f64,
    pub hits: u64,
    pub bins: Vec<(u64, f64, u64)>,
}

impl Calibration {
    fn add(&mut self, p: f64, met: bool) {
        if self.bins.is_empty() {
            self.bins = vec![(0, 0.0, 0); 10];
        }
        let o = if met { 1.0 } else { 0.0 };
        self.n += 1;
        self.brier_sum += (p - o) * (p - o);
        self.hits += o as u64;
        let b = ((p * 10.0).floor() as usize).min(9);
        self.bins[b].0 += 1;
        self.bins[b].1 += p;
        self.bins[b].2 += o as u64;
    }

    pub fn value(&self) -> Value {
        gates::calibration_value(self.n, self.brier_sum, self.hits, &self.bins)
    }
}

/// How many arrivals per channel and facts per key the register keeps.
const HISTORY: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Registers {
    pub todo: BTreeMap<String, Todo>,
    pub projects: BTreeMap<String, Project>,
    pub questions: BTreeMap<String, Question>,
    pub expectations: BTreeMap<String, Expectation>,
    pub calibration: Calibration,
    /// The carried note as last written, with its turn.
    pub note: Option<(u64, String)>,
    /// World facts by key: (arrived, value), newest last.
    pub facts: BTreeMap<String, VecDeque<(Millis, Value)>>,
    /// Arrival times by channel, newest last.
    pub arrivals: BTreeMap<String, VecDeque<Millis>>,
    pub outbox_total: u64,
    next_id: u64,
}

fn push_bounded<T>(q: &mut VecDeque<T>, x: T) {
    q.push_back(x);
    while q.len() > HISTORY {
        q.pop_front();
    }
}

impl Registers {
    /// A fresh id for a register entry: `<prefix><n>`, from the record order.
    pub fn next_id(&self, prefix: &str) -> String {
        format!("{prefix}{}", self.next_id + 1)
    }

    pub fn apply(&mut self, l: &Line) {
        let id = || l.str("id").to_string();
        match l.kind.as_str() {
            "todo.added" => {
                self.next_id += 1;
                self.todo.insert(
                    id(),
                    Todo {
                        text: l.str("text").into(),
                        created_at: l.at,
                        done: false,
                        priority: l.get("priority").as_f64(),
                    },
                );
            }
            "todo.done" => {
                if let Some(t) = self.todo.get_mut(l.str("id")) {
                    t.done = true;
                }
            }
            "project.added" => {
                self.next_id += 1;
                self.projects.insert(
                    id(),
                    Project {
                        title: l.str("title").into(),
                        why: l.str("why").into(),
                        status: l.str("status").into(),
                        created_at: l.at,
                        last_step_at: l.at,
                        priority: l.get("priority").as_f64(),
                    },
                );
            }
            "kernel.commit" | "kernel.progress" => {
                if let Some(p) = self.projects.get_mut(l.str("project")) {
                    p.status = "active".into();
                    p.last_step_at = l.at;
                }
            }
            "kernel.release" => {
                if let Some(p) = self.projects.get_mut(l.str("project")) {
                    p.status = l.str("outcome").into();
                    p.last_step_at = l.at;
                }
            }
            "question.added" => {
                self.next_id += 1;
                self.questions.insert(
                    id(),
                    Question {
                        text: l.str("text").into(),
                        created_at: l.at,
                        open: true,
                        priority: l.get("priority").as_f64(),
                    },
                );
            }
            "question.closed" => {
                if let Some(q) = self.questions.get_mut(l.str("id")) {
                    q.open = false;
                }
            }
            "note.written" => self.note = Some((l.u64("turn"), l.str("text").into())),
            "expectation.made" => {
                self.next_id += 1;
                let Ok(check) = serde_json::from_value::<Check>(l.get("check").clone()) else {
                    return;
                };
                self.expectations.insert(
                    id(),
                    Expectation {
                        claim: l.str("claim").into(),
                        about: l.str("about").into(),
                        warrant: l.str("warrant").into(),
                        p: l.f64("p"),
                        made_at: l.at,
                        turn: l.u64("turn"),
                        due: l.i64("due"),
                        check,
                        state: ExpState::Open,
                        surprise: None,
                    },
                );
            }
            "expectation.revised" => {
                if let Some(e) = self.expectations.get_mut(l.str("id")) {
                    e.p = l.f64("p");
                }
            }
            "expectation.settled" => {
                if let Some(e) = self.expectations.get_mut(l.str("id")) {
                    e.state = match l.str("state") {
                        "met" => ExpState::Met,
                        "missed" => ExpState::Missed,
                        _ => ExpState::Void,
                    };
                    e.surprise = l.get("surprise").as_f64();
                    if e.state != ExpState::Void {
                        let (p, met) = (e.p, e.state == ExpState::Met);
                        self.calibration.add(p, met);
                    }
                }
            }
            "stimulus.accepted" => {
                let item = l.get("item");
                let at = item["at"].as_i64().unwrap_or(l.at);
                if let Some(ch) = item["channel"].as_str() {
                    push_bounded(self.arrivals.entry(ch.into()).or_default(), at);
                }
                if item["kind"] == "world"
                    && let Some(key) = item["fact"]["key"].as_str()
                {
                    push_bounded(
                        self.facts.entry(key.into()).or_default(),
                        (at, item["fact"]["value"].clone()),
                    );
                }
            }
            "outbox.queued" => self.outbox_total += 1,
            _ => {}
        }
    }

    /// Open expectations due within `ms` of `now`.
    pub fn expectations_due(&self, now: Millis, ms: Millis) -> usize {
        self.expectations
            .values()
            .filter(|e| e.state == ExpState::Open && e.due <= now + ms)
            .count()
    }

    /// Settle what can be settled at `now`: decidable checks from the
    /// register's own facts and arrivals, judged ones by `judge` at due.
    pub fn settle(&self, now: Millis, judge: Option<&dyn Judge>) -> Vec<Settlement> {
        let mut out = Vec::new();
        for (id, e) in &self.expectations {
            if e.state != ExpState::Open {
                continue;
            }
            let holds = match &e.check {
                Check::WorldFact { key, equals } => self.facts.get(key).is_some_and(|h| {
                    h.iter()
                        .any(|(at, v)| *at >= e.made_at && *at <= e.due && v == equals)
                }),
                Check::StimulusFrom { channel } => self
                    .arrivals
                    .get(channel)
                    .is_some_and(|h| h.iter().any(|at| *at >= e.made_at && *at <= e.due)),
                Check::Judged { principal } => {
                    if now < e.due {
                        continue;
                    }
                    let verdict = judge
                        .filter(|j| j.name() == principal)
                        .and_then(|j| j.judge(id, e));
                    let by = format!("judge:{principal}");
                    out.push(match verdict {
                        Some(met) => Settlement::new(id, e, met, by, &self.calibration),
                        None => Settlement::void(id, e, by, &self.calibration),
                    });
                    continue;
                }
            };
            if holds {
                out.push(Settlement::new(id, e, true, "host".into(), &self.calibration));
            } else if now > e.due {
                out.push(Settlement::new(id, e, false, "host".into(), &self.calibration));
            }
        }
        out
    }
}

/// A principal that settles judged expectations. It is never the agent:
/// it sees the claim and its warrant, not the agent's reasoning.
pub trait Judge: Send + Sync {
    fn name(&self) -> &str;
    /// Met, missed, or `None` when it cannot say (the expectation is void).
    fn judge(&self, id: &str, e: &Expectation) -> Option<bool>;
}

/// A host settlement of one expectation. Built only here; written only
/// through the record's sealed path.
#[derive(Debug, Clone, PartialEq)]
pub struct Settlement {
    id: String,
    body: Value,
}

impl Settlement {
    fn new(id: &str, e: &Expectation, met: bool, by: String, before: &Calibration) -> Self {
        let mut after = before.clone();
        after.add(e.p, met);
        let state = if met { "met" } else { "missed" };
        Self {
            id: id.into(),
            body: json!({
                "id": id,
                "state": state,
                "p": e.p,
                "surprise": gates::surprise(e.p, met),
                "settled_by": by,
                "calibration": after.value(),
            }),
        }
    }

    fn void(id: &str, e: &Expectation, by: String, before: &Calibration) -> Self {
        Self {
            id: id.into(),
            body: json!({
                "id": id,
                "state": "void",
                "p": e.p,
                "settled_by": by,
                "calibration": before.value(),
            }),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn state(&self) -> &str {
        self.body["state"].as_str().unwrap_or("")
    }

    pub fn surprise(&self) -> Option<f64> {
        self.body["surprise"].as_f64()
    }

    pub(crate) fn into_body(self) -> Value {
        self.body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(seq: u64, at: Millis, kind: &str, body: Value) -> Line {
        let Value::Object(m) = body else { panic!() };
        Line {
            seq,
            at,
            kind: kind.into(),
            body: m,
        }
    }

    #[test]
    fn decidable_expectations_settle_from_the_record() {
        let mut r = Registers::default();
        r.apply(&line(1, 10, "expectation.made", json!({"id": "e1", "p": 0.8, "due": 100, "turn": 1,
            "check": {"world_fact": {"key": "build", "equals": "green"}}})));
        r.apply(&line(2, 10, "expectation.made", json!({"id": "e2", "p": 0.3, "due": 50, "turn": 1,
            "check": {"stimulus_from": {"channel": "owner"}}})));
        assert!(r.settle(20, None).is_empty());
        r.apply(&line(3, 40, "stimulus.accepted", json!({"item": {"id": "w", "kind": "world", "role": "host",
            "channel": "world", "at": 40, "text": "", "fact": {"key": "build", "value": "green"}}})));
        let s = r.settle(41, None);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].id(), s[0].state()), ("e1", "met"));
        let s = r.settle(51, None);
        assert!(s.iter().any(|x| x.id() == "e2" && x.state() == "missed"));
    }
}
