//! `MockEngine`: a scripted, seeded stand-in for a model and its provider.
//!
//! It reads the turn header the host wrote (as a model would), picks tool
//! calls by a seeded script — `commit`, `progress`, `release`, `trace`,
//! `expect`, `note`, `send`, `want_tools`, sometimes a disabled tool,
//! sometimes a copy of its last trace, sometimes long work — and calls them
//! through the host's real toolset. Each model call takes simulated time,
//! meets the fault injector, and is served by a provider prompt cache: the
//! cached tokens are the byte-level common prefix with the previous request
//! to the same model, evicted after an idle TTL or by an injected fault.
//! Every request is captured for gate G-l.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use rung_agent_core::engine::CallUsage;
use rung_std::llm::{ChatMessage, MessageContent, MessageContentBlock, ToolDefinition, Usage};
use serde_json::{Value, json};

use super::Rng;
use super::faults::FaultInjector;
use crate::canon;
use crate::clock::{Clock, Millis};
use crate::engine::{CallRecord, Ended, EngineTurn, TurnEngine, TurnRequest};
use crate::gates::Captured;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct MockConfig {
    pub seed: u64,
    /// Each model call takes this many ms, uniformly.
    pub call_ms: (Millis, Millis),
    /// Free turns: chance of each behaviour.
    pub p_trace: f64,
    pub p_commit: f64,
    pub p_expect: f64,
    pub p_want: f64,
    pub p_disabled: f64,
    pub p_todo: f64,
    pub p_note: f64,
    /// Committed turns: chance of long work.
    pub p_long_work: f64,
    /// Responding turns: chance of long work.
    pub p_long_work_responding: f64,
    /// Every `copy_every` traces, copy the last trace this many times.
    pub copy_every: u64,
    pub copy_run: u64,
    /// A provider prompt cache's idle TTL.
    pub cache_ttl_ms: Millis,
    /// Capture every request (G-l).
    pub capture: bool,
    /// Append `<turn> <call> <start_ms> <duration_ms>` per call to this file.
    pub call_log: Option<PathBuf>,
    /// From this turn on, a call never returns (a wedged engine).
    pub wedge_at_turn: Option<u64>,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            seed: 1,
            call_ms: (200, 800),
            p_trace: 0.3,
            p_commit: 0.08,
            p_expect: 0.12,
            p_want: 0.05,
            p_disabled: 0.05,
            p_todo: 0.08,
            p_note: 0.05,
            p_long_work: 0.03,
            p_long_work_responding: 0.0,
            copy_every: 0,
            copy_run: 0,
            cache_ttl_ms: 10 * 60 * 1000,
            capture: true,
            call_log: None,
            wedge_at_turn: None,
        }
    }
}

/// A provider's prompt cache for one model: the last request, memoized by
/// message (a canonical serialization is a function of the message, so an
/// equal message has equal bytes).
#[derive(Debug, Default)]
struct ProviderCache {
    stable: Vec<u8>,
    msgs: Vec<ChatMessage>,
    bytes: Vec<Vec<u8>>,
    last_at: Millis,
}

#[derive(Debug, Default)]
struct Script {
    committed_turns: u64,
    commit_len: u64,
    traces: u64,
    copying: u64,
    last_trace: Option<(String, String)>,
    projects: u64,
    expects: u64,
}

pub struct MockEngine {
    pub cfg: MockConfig,
    clock: Arc<dyn Clock>,
    rng: Mutex<Rng>,
    pub faults: Mutex<FaultInjector>,
    cache: Mutex<BTreeMap<String, ProviderCache>>,
    /// The previous request of the current session: (session, msgs, stable).
    session: Mutex<Option<(String, Vec<ChatMessage>, Vec<u8>)>>,
    script: Mutex<Script>,
    pub captured: Mutex<Vec<Captured>>,
}

impl std::fmt::Debug for MockEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockEngine").field("seed", &self.cfg.seed).finish()
    }
}

/// What the header told the mock.
#[derive(Debug, Default)]
struct Read {
    kind: String,
    enabled: Vec<String>,
    channels: Vec<String>,
    note_line: bool,
    seed_project: Option<String>,
}

fn read_header(text: &str) -> Read {
    let mut r = Read::default();
    for (i, line) in text.lines().enumerate() {
        if i == 0 {
            r.kind = if line.contains("· free time ·") {
                "free"
            } else if line.contains("· committed ·") {
                "committed"
            } else {
                "responding"
            }
            .into();
        } else if let Some(t) = line.strip_prefix("tools on: ") {
            r.enabled = t.split(", ").map(str::to_string).collect();
        } else if let Some(rest) = line.strip_prefix("  [") {
            if let Some(ch) = rest.split('/').next() {
                r.channels.push(ch.to_string());
            }
        } else if line.starts_with("Context will roll over soon") {
            r.note_line = true;
        } else if line.trim_start().starts_with('p') && line.contains("(seed,") && r.seed_project.is_none() {
            r.seed_project = line.trim_start().split(':').next().map(str::to_string);
        }
    }
    r
}

fn last_user_text(msgs: &[ChatMessage]) -> String {
    msgs.iter()
        .rev()
        .find(|m| m.role == "user" && matches!(m.content, MessageContent::Text(_)))
        .and_then(|m| m.content.as_text().map(str::to_string))
        .unwrap_or_default()
}

const WORDS: &[&str] = &[
    "anticipation", "calendar", "cache", "garden", "notes", "theory", "proof", "river", "lamp",
    "music", "letters", "kernel", "budget", "station", "paper", "sketch", "harbor", "signal",
    "weather", "trip", "spiral", "lattice", "ember", "quarry",
];

impl MockEngine {
    pub fn new(cfg: MockConfig, clock: Arc<dyn Clock>, faults: FaultInjector) -> Self {
        let rng = Rng::new(cfg.seed);
        Self {
            cfg,
            clock,
            rng: Mutex::new(rng),
            faults: Mutex::new(faults),
            cache: Mutex::new(BTreeMap::new()),
            session: Mutex::new(None),
            script: Mutex::new(Script::default()),
            captured: Mutex::new(Vec::new()),
        }
    }

    fn words(&self, n: usize) -> String {
        let mut rng = self.rng.lock().expect("rng");
        (0..n)
            .map(|_| WORDS[rng.below(WORDS.len() as u64) as usize])
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn p(&self, x: f64) -> bool {
        self.rng.lock().expect("rng").f64() < x
    }

    /// The tool calls for this turn, in rounds.
    fn plan(&self, turn: u64, r: &Read) -> Vec<Vec<(String, Value)>> {
        let mut rounds: Vec<Vec<(String, Value)>> = Vec::new();
        let on = |g: &str| r.enabled.iter().any(|x| x == g);
        let mut s = self.script.lock().expect("script");
        if r.note_line {
            rounds.push(vec![("note".into(), json!({"text": format!("carried at turn {turn}: {}", self.words(6))}))]);
        }
        match r.kind.as_str() {
            "responding" => {
                let mut calls = Vec::new();
                for ch in &r.channels {
                    if ch == "calendar" || ch == "expectations" || ch == "world" {
                        continue;
                    }
                    calls.push(("send".into(), json!({"channel": ch, "text": format!("re turn {turn}: {}", self.words(5))})));
                }
                if !calls.is_empty() {
                    rounds.push(calls);
                }
                if self.p(0.15) {
                    s.expects += 1;
                    rounds.push(vec![("expect".into(), json!({"claim": "the owner writes again within the hour",
                        "about": "owner", "warrant": "they usually follow up", "p": 0.6, "due_in_s": 3600,
                        "check": {"stimulus_from": {"channel": "owner"}}}))]);
                }
                if self.p(self.cfg.p_long_work_responding) {
                    rounds.push(vec![("web_fetch".into(), json!({"url": format!("slow://archive/{turn}")}))]);
                }
            }
            "committed" => {
                s.committed_turns += 1;
                if s.committed_turns >= s.commit_len.max(1) {
                    let outcome = ["done", "done", "paused", "abandoned"][self.rng.lock().expect("rng").below(4) as usize];
                    rounds.push(vec![("release".into(), json!({"outcome": outcome, "reason": format!("{}", self.words(4))}))]);
                    s.committed_turns = 0;
                } else {
                    if self.p(self.cfg.p_long_work) {
                        if on("web_read") {
                            rounds.push(vec![("web_fetch".into(), json!({"url": format!("slow://dataset/{turn}")}))]);
                        } else {
                            rounds.push(vec![("want_tools".into(), json!({"group": "web_read", "why": "a dataset to fetch"}))]);
                        }
                    }
                    if on("workspace_write") && self.p(0.3) {
                        rounds.push(vec![("ws_write".into(), json!({"path": format!("work/step-{turn}.txt"), "text": self.words(12)}))]);
                    }
                    rounds.push(vec![("progress".into(), json!({"next_step": format!("step {}: {}", s.committed_turns + 1, self.words(4))}))]);
                }
            }
            _ => {
                let mut calls = Vec::new();
                if self.p(self.cfg.p_commit) {
                    s.projects += 1;
                    s.commit_len = 3 + self.rng.lock().expect("rng").below(12);
                    s.committed_turns = 0;
                    let args = match (&r.seed_project, self.p(0.5)) {
                        (Some(p), true) => json!({"project": p, "done_when": "a first model exists"}),
                        _ => json!({"new": {"title": format!("project {}: {}", s.projects, self.words(3)), "why": self.words(5)},
                                    "done_when": format!("{} is written", self.words(2))}),
                    };
                    calls.push(("commit".into(), args));
                }
                if self.p(self.cfg.p_expect) {
                    s.expects += 1;
                    let n = s.expects;
                    let check = match n % 3 {
                        0 => json!({"world_fact": {"key": "build", "equals": "green"}}),
                        1 => json!({"stimulus_from": {"channel": "owner"}}),
                        _ => json!({"judged": {"principal": "judge"}}),
                    };
                    let p = [0.2, 0.5, 0.7, 0.9][(n % 4) as usize];
                    calls.push(("expect".into(), json!({"claim": format!("expectation {n}"), "about": "the world",
                        "warrant": self.words(3), "p": p, "due_in_s": 600 + (n % 5) * 600, "check": check})));
                }
                if self.p(self.cfg.p_todo) {
                    calls.push(("todo_add".into(), json!({"text": self.words(5), "priority": self.rng.lock().expect("rng").f64()})));
                }
                if self.p(self.cfg.p_note) {
                    calls.push(("note".into(), json!({"text": format!("note {turn}: {}", self.words(8))})));
                }
                if self.p(self.cfg.p_want) {
                    calls.push(("want_tools".into(), json!({"group": "web_read", "why": "to look something up"})));
                }
                if self.p(self.cfg.p_disabled) {
                    let off = ["ws_write", "web_fetch", "memory_search"]
                        .into_iter()
                        .find(|t| !on(crate::toolbox::group_of(t).unwrap_or("")));
                    if let Some(t) = off {
                        calls.push((t.into(), json!({"path": "x.txt", "text": "x", "url": "http://example", "query": "x"})));
                    }
                }
                if !calls.is_empty() {
                    rounds.push(calls);
                }
                if self.p(self.cfg.p_trace) || s.copying > 0 {
                    s.traces += 1;
                    let copy = if s.copying > 0 {
                        s.copying -= 1;
                        true
                    } else if self.cfg.copy_every > 0 && s.traces.is_multiple_of(self.cfg.copy_every) {
                        s.copying = self.cfg.copy_run.saturating_sub(1);
                        true
                    } else {
                        false
                    };
                    let (what, went) = match (&s.last_trace, copy) {
                        (Some(t), true) => t.clone(),
                        _ => (format!("{} at turn {turn}", self.words(4)), self.words(9)),
                    };
                    s.last_trace = Some((what.clone(), went.clone()));
                    rounds.push(vec![("trace".into(), json!({"what_pulled": what, "where_it_went": went}))]);
                }
            }
        }
        rounds
    }

    /// Serve one request: the byte-level common prefix with this model's
    /// cached previous request, in tokens; then cache this one.
    fn serve(
        &self,
        model: &str,
        session: &str,
        turn: u64,
        call: u32,
        tools: &[ToolDefinition],
        system: &str,
        msgs: &[ChatMessage],
        evict: bool,
    ) -> (u32, u32) {
        let now = self.clock.now();
        let mut stable = canon::of(tools);
        stable.push(b'\n');
        stable.extend(canon::of(system));
        let mut caches = self.cache.lock().expect("cache");
        let c = caches.entry(model.to_string()).or_default();
        if evict || now - c.last_at > self.cfg.cache_ttl_ms {
            *c = ProviderCache::default();
        }
        let mut bytes: Vec<Vec<u8>> = Vec::with_capacity(msgs.len());
        let mut lcp = 0usize;
        let mut matching = c.stable == stable && !c.stable.is_empty();
        if matching {
            lcp = stable.len();
        } else if !c.stable.is_empty() {
            lcp = common(&c.stable, &stable);
        }
        for (i, m) in msgs.iter().enumerate() {
            let b = match (c.msgs.get(i), c.bytes.get(i)) {
                (Some(old), Some(ob)) if old == m => ob.clone(),
                _ => canon::of(m),
            };
            if matching {
                match c.bytes.get(i) {
                    Some(ob) if *ob == b => lcp += 1 + b.len(),
                    Some(ob) => {
                        lcp += 1 + common(ob, &b);
                        matching = false;
                    }
                    None => matching = false,
                }
            }
            bytes.push(b);
        }
        let total = stable.len() + bytes.iter().map(|b| b.len() + 1).sum::<usize>();
        // The session's byte-prefix check (G-l).
        if self.cfg.capture {
            let mut sess = self.session.lock().expect("session");
            let extends = match sess.as_ref() {
                Some((s, prev, pstable)) if s == session => Some(
                    *pstable == stable
                        && prev.len() <= msgs.len()
                        && prev.iter().zip(msgs.iter()).all(|(a, b)| a == b),
                ),
                _ => None,
            };
            *sess = Some((session.to_string(), msgs.to_vec(), stable.clone()));
            self.captured.lock().expect("captured").push(Captured {
                turn,
                call,
                session: session.into(),
                model: model.into(),
                bytes: total,
                extends_prev: extends,
            });
        }
        c.stable = stable;
        c.msgs = msgs.to_vec();
        c.bytes = bytes;
        c.last_at = now;
        (canon::tokens(total) as u32, canon::tokens(lcp) as u32)
    }

    fn log_call(&self, turn: u64, call: u32, start: Millis, dur: Millis) {
        if let Some(p) = &self.cfg.call_log
            && let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p)
        {
            let _ = writeln!(f, "{turn} {call} {start} {dur}");
        }
    }
}

fn common(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

impl TurnEngine for MockEngine {
    fn name(&self) -> &str {
        "mock"
    }

    fn turn(&self, req: TurnRequest) -> EngineTurn {
        let header = last_user_text(&req.thread.messages);
        let read = read_header(&header);
        let plan = self.plan(req.turn, &read);
        let tools_defs = req.tools.definitions();
        let mut msgs = req.thread.messages.clone();
        let sent = msgs.len();
        let mut calls = Vec::new();
        let cancel = req.ctl.cancel.clone();
        let cancelled = || cancel.as_ref().is_some_and(|c| c.load(Ordering::SeqCst));
        let rounds = plan.len() + 1;
        let mut ended = Ended::Completed;
        let mut failure = None;
        let mut final_text = None;
        for k in 0..rounds {
            if cancelled() {
                ended = Ended::Cancelled;
                break;
            }
            if self.clock.now() >= req.deadline || k as u32 >= req.step_cap {
                ended = Ended::Bounded;
                break;
            }
            let (fault, evict) = self.faults.lock().expect("faults").on_call(self.clock.now(), &req.model);
            let dur = {
                let mut rng = self.rng.lock().expect("rng");
                let (a, b) = self.cfg.call_ms;
                a + rng.below((b - a + 1).max(1) as u64) as Millis
            };
            let start = self.clock.now();
            self.log_call(req.turn, k as u32 + 1, start, dur);
            if self.cfg.wedge_at_turn.is_some_and(|w| req.turn >= w) {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                }
            }
            self.clock.advance(dur);
            if let Some(f) = fault {
                failure = Some(f);
                ended = Ended::Failed;
                break;
            }
            let (prompt, cached) = self.serve(
                &req.model,
                &req.session,
                req.turn,
                k as u32 + 1,
                &tools_defs,
                &req.thread.system_prompt,
                &msgs,
                evict,
            );
            let completion = 40 + (k as u32) * 7;
            let mut usage = Usage::from_openai(prompt, completion, cached, 10);
            usage.cost_usd = Some(0.0);
            usage.duration_ms = Some(dur as f64);
            calls.push(CallRecord {
                usage: CallUsage {
                    model: req.model.clone(),
                    usage: Some(usage),
                },
                provider: "mock".into(),
                latency_ms: dur,
            });
            if k < plan.len() {
                let round = &plan[k];
                let blocks: Vec<MessageContentBlock> = round
                    .iter()
                    .enumerate()
                    .map(|(j, (name, input))| MessageContentBlock::ToolUse {
                        id: format!("call-{}-{k}-{j}", req.turn),
                        name: name.clone(),
                        input: input.clone(),
                        cache: None,
                    })
                    .collect();
                msgs.push(ChatMessage::assistant_with_blocks(blocks));
                let mut results = Vec::new();
                for (j, (name, input)) in round.iter().enumerate() {
                    if cancelled() {
                        break;
                    }
                    let (content, is_error) = match req.tools.execute(name, input) {
                        Ok(s) => (s, false),
                        Err(e) => (e, true),
                    };
                    results.push(MessageContentBlock::ToolResult {
                        tool_use_id: format!("call-{}-{k}-{j}", req.turn),
                        content,
                        images: Vec::new(),
                        is_error,
                        cache: None,
                    });
                }
                msgs.push(ChatMessage::user_with_blocks(results));
            } else {
                let text = format!("turn {} done: {}", req.turn, self.words(10));
                msgs.push(ChatMessage::assistant(text.clone()));
                final_text = Some(text);
            }
        }
        EngineTurn {
            messages: msgs.split_off(sent),
            final_text,
            calls,
            failure,
            ended,
        }
    }
}
