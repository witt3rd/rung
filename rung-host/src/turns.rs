//! The turns door's library (host API, S8): the record folded into turns, once,
//! on the host, so a screen lays turns out and folds nothing.
//!
//! A turn exists when the record holds its `turn.started` line. Two reads of
//! the record answer a page: the first keeps a few numbers per turn (which turns
//! exist, where each ran, which items each boundary took) and picks the window;
//! the second keeps the texts of the turns in the window and of no others. So
//! memory is the size of the turns asked for, never of the record. Every text
//! is whole and passes through the redactor; nothing here writes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::read::{Damaged, Door, DoorError, check_state_dir, fleet_redactor, locate, scan};
use crate::record::Line;
use crate::redact::Redactor;
use crate::registry::Word;

/// A page of turns when the caller names no size. A default, not a bound.
pub const DEFAULT_LIMIT: u64 = 50;

/// Which turns a list asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnsQuery {
    pub limit: u64,
    /// The page holds the newest turns numbered below this.
    pub before: Option<u64>,
    /// The page holds the oldest turns numbered above this (oldest first).
    pub after: Option<u64>,
}

impl Default for TurnsQuery {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            before: None,
            after: None,
        }
    }
}

fn bad(message: impl Into<String>) -> DoorError {
    DoorError::new(400, "bad_request", message)
}

fn whole(name: &str, v: &str) -> Door<u64> {
    if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad(format!("{name} must be a whole number, not {v:?}")));
    }
    Ok(v.parse().unwrap_or(u64::MAX))
}

impl TurnsQuery {
    /// Parse the query's decoded pairs. An unknown name, a repeated name, a
    /// value that is not a whole number, a `limit` of 0, and `before` with
    /// `after` are 400. A number too big for 64 bits is the biggest one.
    pub fn parse(pairs: &[(String, String)]) -> Door<Self> {
        let mut q = Self::default();
        let mut seen: Vec<&str> = Vec::new();
        for (k, v) in pairs {
            if seen.contains(&k.as_str()) {
                return Err(bad(format!("{k} is given twice")));
            }
            seen.push(k);
            match k.as_str() {
                "limit" => {
                    q.limit = whole(k, v)?;
                    if q.limit == 0 {
                        return Err(bad("limit must be at least 1"));
                    }
                }
                "before" => q.before = Some(whole(k, v)?),
                "after" => q.after = Some(whole(k, v)?),
                other => {
                    return Err(bad(format!(
                        "no such parameter {other:?} (limit, before, after)"
                    )));
                }
            }
        }
        if q.before.is_some() && q.after.is_some() {
            return Err(bad("give before or after, not both"));
        }
        Ok(q)
    }
}

/// The turn number of the path `/v1/turns/{n}` (a whole number; no `/context`).
pub fn parse_turn(segment: &str) -> Door<u64> {
    whole("turn", segment)
}

/// The turn segment of the path `/v1/turns/{n}`, or `None` for any other path.
pub fn turn_of_path(path: &str) -> Option<&str> {
    let n = path.strip_prefix("/v1/turns/")?;
    (!n.is_empty() && !n.contains('/')).then_some(n)
}

// ─── the first read: which turns, where they ran ────────────────────────────

/// What the first read keeps: a few numbers per turn.
#[derive(Debug, Default)]
struct Index {
    /// Every started turn.
    started: BTreeSet<u64>,
    /// Where each turn ran, by line number: from its `turn.started` to its
    /// `turn.ended`, or to where the next turn started when it never ended.
    span: BTreeMap<u64, (u64, u64)>,
    ended: BTreeSet<u64>,
    /// The item ids each turn's boundary took.
    admitted: BTreeMap<u64, Vec<String>>,
}

impl Index {
    fn see(&mut self, l: &Line) {
        let Some(n) = l.get("turn").as_u64().filter(|n| *n >= 1) else {
            return;
        };
        match l.kind.as_str() {
            "turn.started" => {
                self.span.insert(n, (l.seq, u64::MAX));
                self.started.insert(n);
            }
            "turn.ended" => {
                if let Some(s) = self.span.get_mut(&n) {
                    s.1 = l.seq;
                }
                self.ended.insert(n);
            }
            "stimulus.admitted" => {
                let ids = l.get("ids").as_array().into_iter().flatten();
                self.admitted
                    .entry(n)
                    .or_default()
                    .extend(ids.filter_map(|i| i.as_str().map(str::to_string)));
            }
            _ => {}
        }
    }

    fn close_spans(&mut self) {
        let order: Vec<u64> = self.span.keys().copied().collect();
        for pair in order.windows(2) {
            let next_from = self.span[&pair[1]].0;
            let s = self.span.get_mut(&pair[0]).expect("a span");
            if s.1 == u64::MAX {
                s.1 = next_from;
            }
        }
    }

    /// The turn that was running at line `seq`, other than `except`.
    fn running_at(&self, seq: u64, except: u64) -> Option<u64> {
        self.span
            .iter()
            .find(|(n, (from, to))| **n != except && seq > *from && seq < *to)
            .map(|(n, _)| *n)
    }
}

// ─── the second read: the turns of the window, whole ────────────────────────

/// One turn as folded.
#[derive(Debug, Clone, Default)]
struct Turn {
    n: u64,
    kind: String,
    mode: Value,
    project: Value,
    model: Value,
    epoch: Value,
    started_at: Option<i64>,
    ended_at: Option<i64>,
    elapsed_ms: Value,
    status: String,
    failure: Value,
    /// Assistant texts of the `turn.log`, trimmed, in order.
    texts: Vec<String>,
    final_text: String,
    asked: Vec<Value>,
    /// From the `turn.log`.
    logged_calls: Vec<Value>,
    /// From `tool.call` lines, used when there is no `turn.log`.
    line_calls: Vec<Value>,
    refused: Vec<Value>,
    logged: bool,
    llm_calls: u64,
    prompt: u64,
    cached: u64,
    completion: u64,
    cost: f64,
    first_seq: u64,
    last_seq: u64,
}

impl Turn {
    fn text(&self) -> String {
        let mut text = self.texts.join("\n\n");
        let f = self.final_text.trim();
        if !f.is_empty() && !text.contains(f) {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str(f);
        }
        text
    }

    fn calls(&self) -> Vec<Value> {
        let base = if self.logged {
            &self.logged_calls
        } else {
            &self.line_calls
        };
        base.iter().chain(&self.refused).cloned().collect()
    }

    fn row(&self) -> Map<String, Value> {
        let calls = self.calls().len();
        let mut m = Map::new();
        m.insert("turn".into(), self.n.into());
        m.insert("kind".into(), self.kind.clone().into());
        m.insert("mode".into(), self.mode.clone());
        m.insert("project".into(), self.project.clone());
        m.insert("model".into(), self.model.clone());
        m.insert("epoch".into(), self.epoch.clone());
        m.insert("started_at".into(), json!(self.started_at));
        m.insert("ended_at".into(), json!(self.ended_at));
        m.insert("elapsed_ms".into(), self.elapsed_ms.clone());
        m.insert("status".into(), self.status.clone().into());
        m.insert("failure".into(), self.failure.clone());
        m.insert("text".into(), self.text().into());
        m.insert(
            "llm".into(),
            json!({"calls": self.llm_calls, "prompt_tokens": self.prompt,
                   "cached_tokens": self.cached, "completion_tokens": self.completion,
                   "cost_usd": self.cost}),
        );
        m.insert("tool_calls".into(), calls.into());
        m.insert("asked".into(), self.asked.len().into());
        m.insert("first_seq".into(), self.first_seq.into());
        m.insert("last_seq".into(), self.last_seq.into());
        m
    }

    fn detail(&self) -> Value {
        let mut m = self.row();
        m.insert("asked_items".into(), Value::Array(self.asked.clone()));
        m.insert("calls".into(), Value::Array(self.calls()));
        Value::Object(m)
    }
}

/// A message's content blocks (a string content has none).
fn blocks(content: &Value) -> &[Value] {
    content.as_array().map_or(&[], Vec::as_slice)
}

/// Fold one `turn.log` line into `t`: the reply's texts, and each tool call
/// with the result the next user message gave it.
fn fold_log(t: &mut Turn, l: &Line) {
    t.logged = true;
    t.texts.clear();
    t.logged_calls.clear();
    let messages = l.get("messages").as_array().map_or(&[][..], Vec::as_slice);
    let mut results: BTreeMap<&str, &Value> = BTreeMap::new();
    for m in messages.iter().filter(|m| m["role"] == "user") {
        for b in blocks(&m["content"])
            .iter()
            .filter(|b| b["type"] == "tool_result")
        {
            if let Some(id) = b["tool_use_id"].as_str() {
                results.insert(id, b);
            }
        }
    }
    for m in messages.iter().filter(|m| m["role"] == "assistant") {
        if let Some(s) = m["content"].as_str() {
            if !s.trim().is_empty() {
                t.texts.push(s.trim().to_string());
            }
            continue;
        }
        for b in blocks(&m["content"]) {
            match b["type"].as_str() {
                Some("text") => {
                    let s = b["text"].as_str().unwrap_or("").trim();
                    if !s.is_empty() {
                        t.texts.push(s.to_string());
                    }
                }
                Some("tool_use") => {
                    let r = b["id"].as_str().and_then(|id| results.get(id));
                    let result = r.map(|r| match &r["content"] {
                        Value::String(s) => Value::String(s.clone()),
                        other => Value::String(other.to_string()),
                    });
                    t.logged_calls.push(json!({
                        "name": b["name"], "group": null,
                        "ok": r.map(|r| r["is_error"] != true),
                        "refused": null, "input": b["input"], "result": result,
                    }));
                }
                _ => {}
            }
        }
    }
}

/// The fold: the record in `dir` read again, the turns of `want` kept whole.
fn fold_window(
    dir: &Path,
    index: &Index,
    want: &BTreeSet<u64>,
    newest: Option<u64>,
    lock_held: bool,
) -> Door<BTreeMap<u64, Turn>> {
    // the items the window's boundaries took: only these are kept as they arrive
    let ids: BTreeSet<&str> = want
        .iter()
        .filter_map(|n| index.admitted.get(n))
        .flatten()
        .map(String::as_str)
        .collect();
    let mut accepted: BTreeMap<String, (Value, u64, i64)> = BTreeMap::new();
    let mut turns: BTreeMap<u64, Turn> = BTreeMap::new();
    scan(dir, |l: &Line| {
        if l.kind == "stimulus.accepted" {
            let item = l.get("item");
            if let Some(id) = item["id"].as_str().filter(|id| ids.contains(id)) {
                accepted.insert(id.to_string(), (item.clone(), l.seq, l.at));
            }
            return;
        }
        let Some(n) = l.get("turn").as_u64().filter(|n| want.contains(n)) else {
            return;
        };
        let t = turns.entry(n).or_insert_with(|| Turn {
            n,
            kind: "free".into(),
            status: "interrupted".into(),
            first_seq: l.seq,
            ..Turn::default()
        });
        t.first_seq = t.first_seq.min(l.seq);
        t.last_seq = t.last_seq.max(l.seq);
        match l.kind.as_str() {
            "turn.started" => {
                t.started_at = Some(l.at);
                if let Some(k) = l.get("turn_kind").as_str() {
                    t.kind = k.to_string();
                }
                t.mode = l.get("mode").clone();
                t.model = l.get("model").clone();
                t.epoch = l.get("epoch").clone();
                t.project = if l.get("project").is_string() {
                    l.get("project").clone()
                } else {
                    Value::Null
                };
            }
            "stimulus.admitted" => {
                for id in l.get("ids").as_array().into_iter().flatten() {
                    let Some(id) = id.as_str() else { continue };
                    let Some((item, seq, at)) = accepted.get(id) else {
                        continue;
                    };
                    let role = item["role"].as_str().unwrap_or("");
                    let from = if role == "owner" {
                        "owner".to_string()
                    } else {
                        item["kind"].as_str().unwrap_or(role).to_string()
                    };
                    let intent = item["intent"]
                        .as_str()
                        .filter(|i| matches!(*i, "next" | "free" | "steer"));
                    t.asked.push(json!({
                        "id": id, "from": from, "role": role, "intent": intent,
                        "text": item["text"].as_str().unwrap_or(""),
                        "sent_at": item["at"].as_i64().unwrap_or(*at),
                        "seen_at": l.at,
                        "during": index.running_at(*seq, n),
                    }));
                }
            }
            "tool.call" => t.line_calls.push(json!({
                "name": l.get("name"), "group": l.get("group"), "ok": l.get("ok"),
                "refused": null, "input": null, "result": null,
            })),
            "tool.refused" => t.refused.push(json!({
                "name": l.get("name"), "group": l.get("group"), "ok": false,
                "refused": l.get("why").as_str().unwrap_or("refused"),
                "input": null, "result": l.get("message"),
            })),
            "llm.call" => {
                t.llm_calls += 1;
                t.prompt += l.u64("prompt_tokens");
                t.cached += l.u64("cached_tokens");
                t.completion += l.u64("completion_tokens");
                t.cost += l.f64("cost_usd");
            }
            "turn.log" => fold_log(t, l),
            "turn.ended" => {
                t.ended_at = Some(l.at);
                t.status = match l.str("status") {
                    s @ ("completed" | "bounded") => s.to_string(),
                    "cancelled" => "interrupted".to_string(),
                    _ => "failed".to_string(),
                };
                t.failure = if l.get("failure")["class"].is_string() {
                    l.get("failure").clone()
                } else {
                    Value::Null
                };
                t.elapsed_ms = l.get("elapsed_ms").clone();
                if let Some(k) = l.get("turn_kind").as_str() {
                    t.kind = k.to_string();
                }
                if !l.get("model").is_null() {
                    t.model = l.get("model").clone();
                }
                t.final_text = l.str("final_text").to_string();
            }
            _ => {}
        }
    })?;
    // a turn with no end: running if a host holds the lock and it is the newest
    for (n, t) in &mut turns {
        if !index.ended.contains(n) {
            t.status = if lock_held && Some(*n) == newest {
                "running".into()
            } else {
                "interrupted".into()
            };
        }
    }
    Ok(turns)
}

/// What a door needs from the first read.
struct Looked {
    index: Index,
    damaged: Option<Damaged>,
    lock_held: bool,
    dir: std::path::PathBuf,
}

fn look(registry_dir: &Path, id: &str) -> Door<Looked> {
    let (row, entry) = locate(registry_dir, id)?;
    check_state_dir(&entry.state_dir)?;
    let mut index = Index::default();
    let scanned = scan(&entry.state_dir, |l| index.see(l))?;
    index.close_spans();
    Ok(Looked {
        index,
        damaged: scanned.damaged,
        lock_held: row.word == Word::Running,
        dir: entry.state_dir,
    })
}

fn second(looked: &Looked, want: &BTreeSet<u64>) -> Door<BTreeMap<u64, Turn>> {
    let newest = looked.index.started.last().copied();
    fold_window(&looked.dir, &looked.index, want, newest, looked.lock_held)
}

/// `GET /v1/turns`: one page of instance `id`'s turns. Newest first, or oldest
/// first after a cursor.
pub fn list(registry_dir: &Path, id: &str, q: &TurnsQuery, base: &Redactor) -> Door {
    let (red, env_unset) = fleet_redactor(registry_dir, base);
    let run = || -> Door {
        let looked = look(registry_dir, id)?;
        let all: Vec<u64> = looked.index.started.iter().copied().collect();
        let asc = q.after.is_some();
        let eligible: Vec<u64> = if asc {
            all.iter()
                .copied()
                .filter(|n| *n > q.after.unwrap_or(0))
                .collect()
        } else {
            let mut v: Vec<u64> = all
                .iter()
                .copied()
                .filter(|n| q.before.is_none_or(|b| *n < b))
                .collect();
            v.reverse();
            v
        };
        let take = usize::try_from(q.limit).unwrap_or(usize::MAX);
        let page: Vec<u64> = eligible.iter().copied().take(take).collect();
        let next = (eligible.len() > page.len())
            .then(|| page.last().copied())
            .flatten();
        let want: BTreeSet<u64> = page.iter().copied().collect();
        let turns = second(&looked, &want)?;
        let rows: Vec<Value> = page
            .iter()
            .filter_map(|n| turns.get(n))
            .map(|t| Value::Object(t.row()))
            .collect();
        Ok(json!({
            "turns": rows, "total": all.len(), "limit": q.limit,
            "order": if asc { "asc" } else { "desc" }, "next": next,
            "damaged": looked.damaged.as_ref().map(Damaged::to_json),
            "redaction": {"env_unset": env_unset},
        }))
    };
    run()
        .map(|v| red.redact_value(&v))
        .map_err(|e| e.redacted(&red))
}

/// `GET /v1/turns/{n}`: turn `n` of instance `id`, whole.
pub fn one(registry_dir: &Path, id: &str, n: u64, base: &Redactor) -> Door {
    let (red, env_unset) = fleet_redactor(registry_dir, base);
    let run = || -> Door {
        let looked = look(registry_dir, id)?;
        if !looked.index.started.contains(&n) {
            return Err(DoorError::new(
                404,
                "no_such_turn",
                format!("the record has no turn {n}"),
            ));
        }
        let turns = second(&looked, &BTreeSet::from([n]))?;
        let Some(t) = turns.get(&n) else {
            return Err(DoorError::new(
                404,
                "no_such_turn",
                format!("the record has no turn {n}"),
            ));
        };
        let mut v = t.detail();
        v["damaged"] = json!(looked.damaged.as_ref().map(Damaged::to_json));
        v["redaction"] = json!({"env_unset": env_unset});
        Ok(v)
    };
    run()
        .map(|v| red.redact_value(&v))
        .map_err(|e| e.redacted(&red))
}
