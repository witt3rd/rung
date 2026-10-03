//! The decision desk: the host's mechanical decisions, each typed, bounded,
//! with a rule fallback and a logged provenance.
//!
//! Five families ([`admit`], [`inject`], [`tools`], [`pack`],
//! [`consolidate`]) each implement [`HostQuestion`]: a bounded input built
//! in code, atomic Noul/Choice questions, composition in code, guards that
//! always win, and a no-model rule. The decider is any
//! [`rung_std::decide::Decider`] — Jev, a [`rung_std::decide::Recorded`]
//! replay, or the [`Scripted`] test backend.
//!
//! One ask per boundary covers Admit, Inject and Tools; Pack and
//! Consolidate share a second ask, made only when the pack's mechanical
//! gate opens (or every 20 turns for Consolidate). A decider that is
//! missing, over the spend cap, too slow, `Undecided`, or answers short
//! gives way to the rule, and the record says why. The desk never decides
//! what the agent wants: no question has an answer that means "do X
//! instead".

pub mod admit;
pub mod consolidate;
pub mod inject;
pub mod pack;
pub mod tools;

mod scripted;
pub use scripted::{Scripted, Step};

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rung_std::decide::{Answer, Ask, Decided, Decider, Question};
use serde::Serialize;
use serde_json::{Value, json};

use crate::canon;
use crate::clock::{HOUR, MINUTE, Millis, day};
use crate::record::Line;

/// Jev's list price per input token (output is free).
pub const JEV_USD_PER_INPUT_TOKEN: f64 = 0.042e-6;

/// One family of host decisions.
pub trait HostQuestion {
    /// `admit`, `inject`, `tools`, `pack`, `consolidate`.
    const ID: &'static str;
    /// The bounded state, built in code.
    type Input: Serialize;
    /// What composition needs beyond the input (the turn kind, ...).
    type Ctx;
    type Choice: Clone + Debug + Serialize + PartialEq;

    /// The atomic questions for `input`, by id.
    fn questions(input: &Self::Input) -> BTreeMap<String, Question>;

    /// The choice from the decider's answers, or `None` when an answer it
    /// needs is missing (the rule then decides).
    fn compose(input: &Self::Input, d: &Decided, k: &Knobs, ctx: &Self::Ctx)
    -> Option<Self::Choice>;

    /// The no-model choice.
    fn rule(input: &Self::Input, k: &Knobs, ctx: &Self::Ctx) -> Self::Choice;

    /// The hard bounds, applied to every choice however it was made.
    fn guard(input: &Self::Input, c: Self::Choice, k: &Knobs, ctx: &Self::Ctx) -> Self::Choice;
}

/// Why the rule decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Why {
    /// No decider is configured.
    NoDecider,
    /// The desk runs rule-only.
    RuleOnly,
    /// The spend cap would be passed.
    Capped,
    /// The decider did not answer within the timeout.
    Timeout,
    /// The decider could not decide.
    Undecided(String),
    /// The answers missed a question this family needs.
    Incomplete,
    /// Shadow mode: the rule decides, the decider is logged.
    Shadow,
    /// This family had no question to ask.
    NothingToAsk,
}

impl Why {
    pub fn label(&self) -> String {
        match self {
            Why::NoDecider => "no_decider".into(),
            Why::RuleOnly => "rule_only".into(),
            Why::Capped => "capped".into(),
            Why::Timeout => "timeout".into(),
            Why::Undecided(u) => format!("undecided:{u}"),
            Why::Incomplete => "incomplete".into(),
            Why::Shadow => "shadow".into(),
            Why::NothingToAsk => "nothing_to_ask".into(),
        }
    }
}

/// Who decided.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum By {
    Jev {
        /// `jev`, `recorded`, `scripted`, `llm`.
        backend: String,
        model: String,
        cost_usd: f64,
    },
    Rule(Why),
}

impl By {
    pub fn to_value(&self) -> Value {
        match self {
            By::Jev {
                backend,
                model,
                cost_usd,
            } => json!({"jev": {"backend": backend, "model": model, "cost_usd": cost_usd}}),
            By::Rule(w) => json!({"rule": w.label()}),
        }
    }

    pub fn is_rule(&self) -> bool {
        matches!(self, By::Rule(_))
    }
}

/// One decision with its provenance. Built only by the desk, so a branch
/// cannot claim the decider's authority by hand.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision<C> {
    choice: C,
    by: By,
    /// The record body of the decision line.
    line: Value,
}

impl<C> Decision<C> {
    pub fn choice(&self) -> &C {
        &self.choice
    }
    pub fn by(&self) -> &By {
        &self.by
    }
    pub fn into_choice(self) -> C {
        self.choice
    }
    /// The `decision.<family>` line body.
    pub fn line(&self) -> &Value {
        &self.line
    }
}

/// How the desk uses its decider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeskMode {
    /// The decider decides, inside the guards.
    Decide,
    /// The rule decides; the decider is asked and logged beside it.
    Shadow,
    /// The rule decides; the decider is not asked.
    RuleOnly,
}

/// Every threshold the families use. Code owns them; a decider never sets
/// one.
#[derive(Debug, Clone, Serialize)]
pub struct Knobs {
    pub admit_now_p: f64,
    pub recall_p: f64,
    pub digest_p: f64,
    pub enable_p: f64,
    pub keep_p: f64,
    pub retain_p: f64,
    pub note_p: f64,
    /// Maximum deferral by item kind, ms.
    pub max_deferral: BTreeMap<String, Millis>,
    pub max_interrupts_per_hour: u32,
    /// A group stays on at least this many turns once enabled.
    pub stay_on_turns: u64,
    /// ... and off at least this many turns once disabled.
    pub stay_off_turns: u64,
    /// A `want_tools` request enables a group for this many turns (rule).
    pub want_turns: u64,
    pub pack_floor: f64,
    pub pack_break_floor: f64,
    pub pack_rule_break: f64,
    pub pack_ceiling: f64,
    pub keep_budget: f64,
    pub consolidate_every: u64,
    pub recall_every_free: u64,
    pub note_age_turns: u64,
    pub max_items: usize,
    pub max_segments: usize,
    pub max_candidates: usize,
}

impl Default for Knobs {
    fn default() -> Self {
        let d = |m: Millis| m;
        Self {
            admit_now_p: 0.6,
            recall_p: 0.5,
            digest_p: 0.5,
            enable_p: 0.5,
            keep_p: 0.6,
            retain_p: 0.5,
            note_p: 0.5,
            max_deferral: [
                ("peer", d(30 * MINUTE)),
                ("expectation", d(2 * HOUR)),
                ("world", d(HOUR)),
                ("calendar", d(HOUR)),
                ("memory", d(HOUR)),
                ("completion", d(30 * MINUTE)),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
            max_interrupts_per_hour: 6,
            stay_on_turns: 3,
            stay_off_turns: 1,
            want_turns: 5,
            pack_floor: 0.40,
            pack_break_floor: 0.25,
            pack_rule_break: 0.60,
            pack_ceiling: 0.85,
            keep_budget: 0.15,
            consolidate_every: 20,
            recall_every_free: 10,
            note_age_turns: 5,
            max_items: 8,
            max_segments: 10,
            max_candidates: 10,
        }
    }
}

/// The desk's spend caps (USD), for a decider that costs money.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SpendCap {
    pub per_day: f64,
    pub per_ask: f64,
}

impl Default for SpendCap {
    fn default() -> Self {
        Self {
            per_day: 0.25,
            per_ask: 0.001,
        }
    }
}

/// The desk.
pub struct DecisionDesk {
    decider: Option<Arc<dyn Decider>>,
    /// What the decider is, for provenance: `jev`, `recorded`, `scripted`.
    backend: String,
    pub mode: DeskMode,
    /// The longest one ask may take. Under the 2 s boundary budget, so the
    /// composition after it still fits.
    pub timeout: Duration,
    pub cap: SpendCap,
    pub knobs: Knobs,
}

impl std::fmt::Debug for DecisionDesk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecisionDesk")
            .field("backend", &self.backend)
            .field("mode", &self.mode)
            .finish()
    }
}

/// The default ask timeout.
pub const ASK_TIMEOUT: Duration = Duration::from_millis(1_900);

impl DecisionDesk {
    pub fn new(decider: Option<Arc<dyn Decider>>, backend: &str, mode: DeskMode) -> Self {
        Self {
            decider,
            backend: backend.into(),
            mode,
            timeout: ASK_TIMEOUT,
            cap: SpendCap::default(),
            knobs: Knobs::default(),
        }
    }

    /// A desk with no decider: every family decides by its rule.
    pub fn rule_only() -> Self {
        Self::new(None, "none", DeskMode::RuleOnly)
    }

    pub fn backend(&self) -> &str {
        &self.backend
    }

    /// Ask the decider once for every question in `questions`, within the
    /// desk's timeout.
    pub fn ask(&self, state: Value, questions: BTreeMap<String, Question>, spent_today: f64) -> Asked {
        self.ask_until(state, questions, spent_today, Instant::now() + self.timeout)
    }

    /// Ask, giving up at `deadline` (a boundary's asks share one budget).
    pub fn ask_until(
        &self,
        state: Value,
        questions: BTreeMap<String, Question>,
        spent_today: f64,
        deadline: Instant,
    ) -> Asked {
        let started = Instant::now();
        let ask = Ask { state, questions };
        let est = ask.estimated_tokens() as f64 * JEV_USD_PER_INPUT_TOKEN;
        let fail = |why: Why, timed_out: bool| Asked {
            result: Err(why),
            est_usd: est,
            cost_usd: 0.0,
            wall_us: started.elapsed().as_micros() as u64,
            timed_out,
            questions: ask.questions.clone(),
        };
        if ask.questions.is_empty() {
            return fail(Why::NothingToAsk, false);
        }
        let Some(decider) = self.decider.clone() else {
            return fail(Why::NoDecider, false);
        };
        if self.mode == DeskMode::RuleOnly {
            return fail(Why::RuleOnly, false);
        }
        if est > self.cap.per_ask || spent_today + est > self.cap.per_day {
            return fail(Why::Capped, false);
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let sent = ask.clone();
        std::thread::spawn(move || {
            let _ = tx.send(decider.decide(&sent));
        });
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(Ok(d)) => Asked {
                cost_usd: d.usage.cost_usd,
                result: Ok(d),
                est_usd: est,
                wall_us: started.elapsed().as_micros() as u64,
                timed_out: false,
                questions: ask.questions,
            },
            Ok(Err(u)) => fail(Why::Undecided(u.to_string()), false),
            Err(_) => fail(Why::Timeout, true),
        }
    }

    /// One family's decision from one ask's outcome.
    pub fn decide<Q: HostQuestion>(
        &self,
        input: &Q::Input,
        ctx: &Q::Ctx,
        asked: &Asked,
        boundary: u64,
        turn: u64,
    ) -> Decision<Q::Choice> {
        let k = &self.knobs;
        let qs = Q::questions(input);
        let rule = Q::rule(input, k, ctx);
        let mut shadow = None;
        let (choice, by) = if qs.is_empty() {
            (rule.clone(), By::Rule(Why::NothingToAsk))
        } else {
            match &asked.result {
                Err(why) => (rule.clone(), By::Rule(why.clone())),
                Ok(d) => {
                    let jev = Q::compose(input, d, k, ctx);
                    let by_jev = By::Jev {
                        backend: self.backend.clone(),
                        model: d.model.clone(),
                        cost_usd: canon::fixed(asked.cost_usd),
                    };
                    match (self.mode, jev) {
                        (DeskMode::Shadow, j) => {
                            shadow = Some(j.map(|c| Q::guard(input, c, k, ctx)));
                            (rule.clone(), By::Rule(Why::Shadow))
                        }
                        (_, Some(c)) => (c, by_jev),
                        (_, None) => (rule.clone(), By::Rule(Why::Incomplete)),
                    }
                }
            }
        };
        let choice = Q::guard(input, choice, k, ctx);
        let rule_guarded = Q::guard(input, rule, k, ctx);
        let answers: BTreeMap<&String, &Answer> = match &asked.result {
            Ok(d) => d
                .answers
                .iter()
                .filter(|(id, _)| qs.contains_key(*id))
                .collect(),
            Err(_) => BTreeMap::new(),
        };
        let input_v = serde_json::to_value(input).unwrap_or(Value::Null);
        let mut line = json!({
            "boundary": boundary,
            "turn": turn,
            "input_hash": canon::hash_value(&input_v),
            "questions_hash": canon::hash(&canon::of(&qs)),
            "questions": qs.len(),
            "answers": serde_json::to_value(&answers).unwrap_or(Value::Null),
            "choice": serde_json::to_value(&choice).unwrap_or(Value::Null),
            "by": by.to_value(),
            "wall_us": asked.wall_us,
        });
        if asked.timed_out {
            line["delayed"] = true.into();
        }
        if let Some(s) = shadow {
            line["jev_choice"] = serde_json::to_value(&s).unwrap_or(Value::Null);
            line["agree"] = (s.as_ref() == Some(&rule_guarded)).into();
        }
        Decision { choice, by, line }
    }
}

/// One ask's outcome, shared by the families it covered.
#[derive(Debug, Clone)]
pub struct Asked {
    pub result: Result<Decided, Why>,
    pub est_usd: f64,
    pub cost_usd: f64,
    pub wall_us: u64,
    pub timed_out: bool,
    pub questions: BTreeMap<String, Question>,
}

impl Asked {
    /// The `desk.ask` line body.
    pub fn line(&self, boundary: u64, families: &[&str], backend: &str) -> Value {
        json!({
            "boundary": boundary,
            "families": families,
            "backend": backend,
            "questions": self.questions.len(),
            "outcome": match &self.result {
                Ok(_) => "answered".to_string(),
                Err(w) => w.label(),
            },
            "est_usd": canon::fixed(self.est_usd),
            "cost_usd": canon::fixed(self.cost_usd),
            "wall_us": self.wall_us,
        })
    }
}

/// P(yes) of a Noul answer, when given.
pub fn noul(d: &Decided, id: &str) -> Option<f64> {
    d.noul(id)
}

/// The chosen option of a Choice answer, when given.
pub fn choice<'a>(d: &'a Decided, id: &str) -> Option<&'a str> {
    d.choice(id).map(|(c, _, _)| c)
}

/// Make a question id safe: letters, digits, `_` and `-`.
pub fn qid(prefix: &str, id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{prefix}_{safe}")
}

// ─── The desk's projection ───────────────────────────────────────────────────

/// A tool group's switch state.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupState {
    pub enabled: bool,
    /// The turn it was last switched.
    pub since_turn: u64,
}

/// A `want_tools` request.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Want {
    pub group: String,
    pub why: String,
    pub turn: u64,
}

/// Something worth offering to long-term memory.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Candidate {
    pub id: String,
    /// `trace`, `outcome`, `settled`, `completion`.
    pub kind: String,
    pub turn: u64,
    pub gist: String,
    pub text: String,
}

/// What the desk's families read from the record.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DeskState {
    /// UTC day of `spent_today`.
    pub day: i64,
    pub spent_today: f64,
    pub groups: BTreeMap<String, GroupState>,
    pub wants: Vec<Want>,
    /// Tool uses by group for the last ten turns, newest last.
    pub uses: VecDeque<BTreeMap<String, u32>>,
    /// When a commitment was interrupted, the last hour's.
    pub interrupts: VecDeque<Millis>,
    pub last_recall_turn: Option<u64>,
    pub recall_hits_last: u64,
    pub last_consolidate_turn: u64,
    pub candidates: Vec<Candidate>,
    /// Requests for groups outside the superset, by group.
    pub outside_requests: BTreeMap<String, u64>,
    /// (cached, prompt) tokens of the last ten model calls.
    pub cache_recent: VecDeque<(u64, u64)>,
}

/// Candidates kept waiting at most.
const MAX_WAITING_CANDIDATES: usize = 50;

impl DeskState {
    pub fn apply(&mut self, l: &Line) {
        let d = day(l.at);
        if d != self.day {
            self.day = d;
            self.spent_today = 0.0;
        }
        while self.interrupts.front().is_some_and(|t| *t < l.at - HOUR) {
            self.interrupts.pop_front();
        }
        let mut candidate = |kind: &str, turn: u64, text: String| {
            self.candidates.push(Candidate {
                id: format!("c{}", l.seq),
                kind: kind.into(),
                turn,
                gist: crate::inbox::gist(&text),
                text: text.chars().take(1_000).collect(),
            });
            if self.candidates.len() > MAX_WAITING_CANDIDATES {
                self.candidates.remove(0);
            }
        };
        match l.kind.as_str() {
            "desk.ask" => self.spent_today += l.f64("cost_usd"),
            "llm.call" => {
                self.cache_recent
                    .push_back((l.u64("cached_tokens"), l.u64("prompt_tokens")));
                while self.cache_recent.len() > 10 {
                    self.cache_recent.pop_front();
                }
            }
            "decision.tools" => {
                let on: Vec<String> = crate::inbox::ids(&l.get("choice")["enabled"]);
                let turn = l.u64("turn");
                let names: Vec<String> = self
                    .groups
                    .keys()
                    .cloned()
                    .chain(on.iter().cloned())
                    .collect();
                for g in names {
                    let want = on.contains(&g);
                    let s = self.groups.entry(g).or_insert(GroupState {
                        enabled: !want,
                        since_turn: turn,
                    });
                    if s.enabled != want {
                        s.enabled = want;
                        s.since_turn = turn;
                    }
                }
            }
            "decision.admit" => {
                if l.get("choice")["interrupts"].as_u64().unwrap_or(0) > 0 {
                    self.interrupts.push_back(l.at);
                }
            }
            "tools.wanted" => {
                self.wants.push(Want {
                    group: l.str("group").into(),
                    why: l.str("why").into(),
                    turn: l.u64("turn"),
                });
                if l.get("outside") == &Value::Bool(true) {
                    *self.outside_requests.entry(l.str("group").into()).or_default() += 1;
                }
                if self.wants.len() > 20 {
                    self.wants.remove(0);
                }
            }
            "turn.started" => {
                self.uses.push_back(BTreeMap::new());
                while self.uses.len() > 10 {
                    self.uses.pop_front();
                }
            }
            "tool.call" => {
                if let Some(m) = self.uses.back_mut() {
                    *m.entry(l.str("group").into()).or_default() += 1;
                }
            }
            "memory.recall" => {
                self.last_recall_turn = Some(l.u64("turn"));
                self.recall_hits_last = l.get("report")["records"].as_u64().unwrap_or(0);
            }
            "decision.consolidate" => {
                self.last_consolidate_turn = l.u64("turn");
                let seen = crate::inbox::ids(&l.get("choice")["considered"]);
                self.candidates.retain(|c| !seen.contains(&c.id));
            }
            "kernel.trace" => candidate(
                "trace",
                l.u64("turn"),
                format!(
                    "Trace: {} → {} {}",
                    l.str("what_pulled"),
                    l.str("where_it_went"),
                    l.str("still_thinking")
                ),
            ),
            "kernel.release" => candidate(
                "outcome",
                l.u64("turn"),
                format!(
                    "Commitment {} released ({}): {}",
                    l.str("project"),
                    l.str("outcome"),
                    l.str("reason")
                ),
            ),
            "expectation.settled" if l.str("state") != "void" => candidate(
                "settled",
                0,
                format!(
                    "Expectation {} {} (p={}, surprise {})",
                    l.str("id"),
                    l.str("state"),
                    l.f64("p"),
                    l.f64("surprise")
                ),
            ),
            "turn.ended" if l.str("status") == "completed" => {
                if let Some(t) = l.get("final_text").as_str().filter(|t| !t.is_empty()) {
                    candidate("completion", l.u64("turn"), t.to_string());
                }
            }
            _ => {}
        }
    }

    /// Cached over prompt tokens across the last ten calls.
    pub fn cache_ratio(&self) -> f64 {
        let (c, p) = self
            .cache_recent
            .iter()
            .fold((0, 0), |(a, b), (c, p)| (a + c, b + p));
        if p == 0 { 0.0 } else { c as f64 / p as f64 }
    }

    pub fn enabled(&self, group: &str) -> bool {
        self.groups.get(group).is_some_and(|g| g.enabled)
    }
}
