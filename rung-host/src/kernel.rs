//! The free-time kernel: what the next turn is for.
//!
//! Free time is the default. With nothing admitted and no commitment, the
//! host runs a free-time turn whose content is the agent's own choice. The
//! agent may `commit` to a project; its turns then continue that project
//! until it calls `release` (or the owner releases it). An admitted item
//! makes the turn a Responding one; afterwards the prior mode resumes.
//!
//! The mode is host state, projected from the record (`kernel.*` lines), so
//! it survives a restart. Only two paths write `kernel.commit` and
//! `kernel.release`: the agent's own tools (`tool_commit`,
//! `tool_release`) and the owner's release ([`owner_release`]). Both go
//! through [`KernelEntry`], whose constructors are private to this module,
//! and the record's sealed path (gate G-c pins the refusal from outside).

use std::collections::{BTreeSet, VecDeque};

use serde::Serialize;
use serde_json::{Value, json};

use crate::clock::{Millis, SECOND};
use crate::core::{Core, Sealed};
use crate::record::Line;

/// The host's mode.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum Mode {
    Free,
    Committed(Commitment),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Commitment {
    pub project: String,
    pub title: String,
    pub done_when: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<Millis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_every: Option<u64>,
    pub since_turn: u64,
    pub since_at: Millis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<String>,
    pub last_progress_turn: u64,
}

/// What one turn is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnKind {
    Responding,
    Committed,
    Free,
}

impl TurnKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TurnKind::Responding => "responding",
            TurnKind::Committed => "committed",
            TurnKind::Free => "free",
        }
    }
}

/// How many recent traces the copy guard compares against.
pub const RECENT_TRACES: usize = 5;
/// A trace this similar to a recent one is a copy.
pub const COPY_SIMILARITY: f64 = 0.8;
/// Consecutive copying turns that make a copy loop.
pub const COPY_LOOP_TURNS: u64 = 3;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KernelState {
    pub mode: Mode,
    /// A free session is running (free turns since the last trace, commit
    /// or release).
    pub free_session_open: bool,
    pub free_session_turns: u64,
    /// Recent traces and final answers, newest last: (turn, text).
    pub recent: VecDeque<(u64, String)>,
    /// Consecutive turns whose trace or answer copied a recent one.
    pub copy_streak: u64,
    /// The turn of the last copy-loop intervention.
    pub last_copy_loop: Option<u64>,
    /// The last turn the copy guard flagged.
    pub last_guard_turn: Option<u64>,
    /// Turns where a commitment ran without `progress`.
    pub turns_since_progress: u64,
    pub commits: u64,
    pub releases: u64,
}

impl Default for KernelState {
    fn default() -> Self {
        Self {
            mode: Mode::Free,
            free_session_open: false,
            free_session_turns: 0,
            recent: VecDeque::new(),
            copy_streak: 0,
            last_copy_loop: None,
            last_guard_turn: None,
            turns_since_progress: 0,
            commits: 0,
            releases: 0,
        }
    }
}

fn push_recent(q: &mut VecDeque<(u64, String)>, turn: u64, text: &str) {
    q.push_back((turn, text.to_string()));
    while q.len() > RECENT_TRACES {
        q.pop_front();
    }
}

impl KernelState {
    pub fn apply(&mut self, l: &Line) {
        match l.kind.as_str() {
            "kernel.commit" => {
                self.commits += 1;
                self.free_session_open = false;
                self.free_session_turns = 0;
                self.turns_since_progress = 0;
                self.mode = Mode::Committed(Commitment {
                    project: l.str("project").into(),
                    title: l.str("title").into(),
                    done_when: l.str("done_when").into(),
                    until: l.get("until").as_i64(),
                    checkpoint_every: l.get("checkpoint_every").as_u64(),
                    since_turn: l.u64("turn"),
                    since_at: l.at,
                    next_step: None,
                    last_progress_turn: l.u64("turn"),
                });
            }
            "kernel.progress" => {
                if let Mode::Committed(c) = &mut self.mode {
                    c.next_step = Some(l.str("next_step").into());
                    c.last_progress_turn = l.u64("turn");
                }
                self.turns_since_progress = 0;
            }
            "kernel.release" => {
                self.releases += 1;
                self.mode = Mode::Free;
                self.free_session_open = false;
                self.free_session_turns = 0;
            }
            "kernel.trace" => {
                self.free_session_open = false;
                self.free_session_turns = 0;
                push_recent(&mut self.recent, l.u64("turn"), &trace_text(&l.body));
            }
            "turn.started" => match l.str("turn_kind") {
                "free" => {
                    self.free_session_open = true;
                    self.free_session_turns += 1;
                }
                "committed" => self.turns_since_progress += 1,
                _ => {}
            },
            "turn.ended" => {
                if let Some(t) = l.get("final_text").as_str()
                    && !t.is_empty()
                {
                    push_recent(&mut self.recent, l.u64("turn"), t);
                }
                if l.get("copied") == &Value::Bool(true) {
                    self.copy_streak += 1;
                } else if l.get("copied") == &Value::Bool(false) {
                    self.copy_streak = 0;
                }
            }
            "copy.guard" => self.last_guard_turn = Some(l.u64("turn")),
            "copy.loop" => {
                self.copy_streak = 0;
                self.last_copy_loop = Some(l.u64("turn"));
            }
            _ => {}
        }
    }

    /// The next turn's kind, given whether anything was admitted now.
    pub fn next(&self, admitted_now: bool) -> TurnKind {
        if admitted_now {
            TurnKind::Responding
        } else if matches!(self.mode, Mode::Committed(_)) {
            TurnKind::Committed
        } else {
            TurnKind::Free
        }
    }

    /// The agent is at a natural break: free, and no free session running.
    pub fn at_break(&self) -> bool {
        matches!(self.mode, Mode::Free) && !self.free_session_open
    }

    /// `free` or `committed:<project>`.
    pub fn mode_label(&self) -> String {
        match &self.mode {
            Mode::Free => "free".into(),
            Mode::Committed(c) => format!("committed:{}", c.project),
        }
    }

    pub fn commitment(&self) -> Option<&Commitment> {
        match &self.mode {
            Mode::Committed(c) => Some(c),
            Mode::Free => None,
        }
    }

    /// The highest similarity of `text` to a recent trace or answer.
    pub fn similarity(&self, text: &str) -> f64 {
        self.recent
            .iter()
            .map(|(_, t)| similarity(text, t))
            .fold(0.0, f64::max)
    }
}

fn trace_text(b: &serde_json::Map<String, Value>) -> String {
    let s = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or("");
    format!(
        "{} {} {}",
        s("what_pulled"),
        s("where_it_went"),
        s("still_thinking")
    )
}

/// Trigram Jaccard similarity of two texts (lowercased, whitespace
/// collapsed). 1.0 for identical non-empty texts.
pub fn similarity(a: &str, b: &str) -> f64 {
    let grams = |s: &str| -> BTreeSet<[char; 3]> {
        let norm: Vec<char> = s
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .collect();
        norm.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
    };
    let (x, y) = (grams(a), grams(b));
    if x.is_empty() || y.is_empty() {
        return 0.0;
    }
    let inter = x.intersection(&y).count() as f64;
    let union = x.union(&y).count() as f64;
    inter / union
}

// ─── The sealed entry ────────────────────────────────────────────────────────

/// A `kernel.commit` or `kernel.release` line. Built only by the agent's
/// tools and the owner's release, in this module.
#[derive(Debug)]
pub struct KernelEntry {
    kind: &'static str,
    body: Value,
}

impl KernelEntry {
    fn commit(body: Value) -> Self {
        Self {
            kind: "kernel.commit",
            body,
        }
    }

    fn release(body: Value) -> Self {
        Self {
            kind: "kernel.release",
            body,
        }
    }
}

impl Sealed for KernelEntry {
    fn kind(&self) -> &'static str {
        self.kind
    }
    fn into_body(self) -> Value {
        self.body
    }
}

// ─── The agent's tools ───────────────────────────────────────────────────────

fn text<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("`{key}` is required"))
}

/// `commit{project | new{title, why}, done_when, until_s?, checkpoint_every?}`.
pub(crate) fn tool_commit(core: &Core, turn: u64, input: &Value) -> Result<String, String> {
    let st = core.state();
    if let Some(c) = st.kernel.commitment() {
        return Err(format!(
            "already committed to `{}`; release it first (one commitment at a time)",
            c.project
        ));
    }
    let done_when = text(input, "done_when")?.to_string();
    let (project, title, why, is_new) =
        if let Some(id) = input.get("project").and_then(Value::as_str) {
            let Some(p) = st.registers.projects.get(id) else {
                return Err(format!("no project `{id}`"));
            };
            if p.status == "done" || p.status == "abandoned" {
                return Err(format!("project `{id}` is {}", p.status));
            }
            (id.to_string(), p.title.clone(), p.why.clone(), false)
        } else if let Some(n) = input.get("new") {
            let title = text(n, "title")?.to_string();
            let why = text(n, "why")?.to_string();
            (st.registers.next_id("p"), title, why, true)
        } else {
            return Err("name a `project` or describe a `new` one".into());
        };
    let until = input
        .get("until_s")
        .and_then(Value::as_i64)
        .filter(|s| *s > 0)
        .map(|s| core.clock.now().saturating_add(s.saturating_mul(SECOND)));
    let checkpoint_every = input.get("checkpoint_every").and_then(Value::as_u64);
    drop(st);
    if is_new {
        core.emit(
            "project.added",
            json!({"id": project, "title": title, "why": why, "status": "active", "turn": turn}),
        );
    }
    core.emit_sealed(KernelEntry::commit(json!({
        "turn": turn,
        "project": project,
        "title": title,
        "why": why,
        "done_when": done_when,
        "until": until,
        "checkpoint_every": checkpoint_every,
        "via": "tool:commit",
        "by": "agent",
    })));
    if let Some(t) = until {
        crate::presence::add_calendar(
            core,
            &format!("commit:{project}"),
            t,
            crate::calendar::Origin::Agent,
            &format!("commitment `{title}`: until passed"),
        );
    }
    Ok(format!(
        "committed to `{project}` ({title}); release it when done"
    ))
}

/// `progress{next_step, note?}`.
pub(crate) fn tool_progress(core: &Core, turn: u64, input: &Value) -> Result<String, String> {
    let project = match core.state().kernel.commitment() {
        Some(c) => c.project.clone(),
        None => return Err("not committed to anything".into()),
    };
    let next_step = text(input, "next_step")?;
    core.emit(
        "kernel.progress",
        json!({"turn": turn, "project": project, "next_step": next_step,
               "note": input.get("note").and_then(Value::as_str)}),
    );
    Ok("progress recorded".into())
}

/// `release{outcome: done|paused|abandoned, reason}`.
pub(crate) fn tool_release(core: &Core, turn: u64, input: &Value) -> Result<String, String> {
    let project = match core.state().kernel.commitment() {
        Some(c) => c.project.clone(),
        None => return Err("not committed to anything".into()),
    };
    let outcome = text(input, "outcome")?;
    if !matches!(outcome, "done" | "paused" | "abandoned") {
        return Err("`outcome` is done, paused or abandoned".into());
    }
    let reason = text(input, "reason")?;
    core.emit_sealed(KernelEntry::release(json!({
        "turn": turn,
        "project": project,
        "outcome": outcome,
        "reason": reason,
        "via": "tool:release",
        "released_by": "agent",
    })));
    crate::presence::remove_calendar(core, &format!("commit:{project}"));
    Ok(format!("released `{project}` ({outcome}); free time again"))
}

/// The owner's one override: release the current commitment.
pub fn owner_release(core: &Core, reason: &str) -> Result<String, String> {
    let (project, turn) = {
        let st = core.state();
        match st.kernel.commitment() {
            Some(c) => (c.project.clone(), st.turn),
            None => return Err("not committed to anything".into()),
        }
    };
    core.emit_sealed(KernelEntry::release(json!({
        "turn": turn,
        "project": project,
        "outcome": "paused",
        "reason": reason,
        "via": "owner:_rung/release",
        "released_by": "owner",
    })));
    crate::presence::remove_calendar(core, &format!("commit:{project}"));
    Ok(format!("released `{project}` by the owner"))
}

/// `trace{what_pulled, where_it_went, still_thinking?}`.
pub(crate) fn tool_trace(core: &Core, turn: u64, input: &Value) -> Result<String, String> {
    let what = text(input, "what_pulled")?;
    let went = text(input, "where_it_went")?;
    let still = input
        .get("still_thinking")
        .and_then(Value::as_str)
        .unwrap_or("");
    let sim = core
        .state()
        .kernel
        .similarity(&format!("{what} {went} {still}"));
    core.emit(
        "kernel.trace",
        json!({"turn": turn, "what_pulled": what, "where_it_went": went,
               "still_thinking": still, "similarity": crate::canon::fixed(sim)}),
    );
    if sim > COPY_SIMILARITY {
        core.emit(
            "copy.guard",
            json!({"turn": turn, "similarity": crate::canon::fixed(sim), "of": "trace"}),
        );
    }
    Ok("trace kept".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarity_is_trigram_jaccard() {
        assert_eq!(similarity("the same words", "The  same words"), 1.0);
        assert!(similarity("cache efficiency", "calendar lateness") < 0.2);
        assert_eq!(similarity("", "x"), 0.0);
    }

    #[test]
    fn the_kernel_picks_the_mode() {
        let mut k = KernelState::default();
        assert_eq!(k.next(false), TurnKind::Free);
        assert!(k.at_break());
        let l = Line::parse(r#"{"seq":1,"at":5,"kind":"kernel.commit","turn":3,"project":"p1","title":"t","done_when":"d"}"#).unwrap();
        k.apply(&l);
        assert_eq!(k.next(false), TurnKind::Committed);
        assert_eq!(k.next(true), TurnKind::Responding);
        assert_eq!(k.mode_label(), "committed:p1");
        let l = Line::parse(
            r#"{"seq":2,"at":6,"kind":"kernel.release","turn":4,"project":"p1","outcome":"done"}"#,
        )
        .unwrap();
        k.apply(&l);
        assert_eq!(k.next(false), TurnKind::Free);
    }
}
