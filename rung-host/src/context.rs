//! The context door (S4): what was sent to the model for one call of one turn.
//!
//! `GET /v1/turns/{n}/context?call=k`. The record keeps each layer of a request
//! once, where it was made (`pack.layers`: the tool definitions and the system
//! text; `epoch.rollover`: the epoch layer; `turn.log`: the header as sent, the
//! recall text on its own, and the turn's messages). This door joins them by
//! the hashes the `llm.call` lines carry and slices the turn's messages at call
//! `k`, giving the stacked **bands** in the order the model sees them: tools,
//! system, epoch, history, message, recall (the turn header is one wire
//! message: its message part first, its recall block after), this turn so far.
//! Each band has its whole text, the host's own size estimate and a cache mark;
//! the provider's counts for the call ride along.
//!
//! It reads the files, so a stopped instance answers like a running one. A
//! record from before the layers were recorded answers with what it has and says
//! so; it is never an error. The answer is **streamed**: a first pass reads the
//! record keeping only sizes and line numbers (a few numbers per turn of the
//! epoch); a second pass writes each band as its line is read, so what is held
//! is one record line, however large the turn, the epoch or the history.
//! Shapes: `docs/rung-host-api.md` (S4).

use std::io::{self, Write};
use std::path::Path;

use rung_std::llm::ChatMessage;
use serde_json::{Map, Value, json};

use crate::canon;
use crate::read::{self, Damaged, Door, DoorError};
use crate::record::Line;
use crate::redact::Redactor;

/// What the door says of a turn that predates the request record.
pub const PREDATES: &str = "this turn predates context recording";

/// A query on the context door.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContextQuery {
    /// The model call of the turn, counting from 1. The first when not given.
    pub call: Option<u64>,
}

impl ContextQuery {
    /// Parse the query's decoded pairs: only `call`, once, a whole number.
    /// Anything else is 400.
    pub fn parse(pairs: &[(String, String)]) -> Door<Self> {
        let mut q = Self::default();
        for (i, (k, v)) in pairs.iter().enumerate() {
            if pairs[..i].iter().any(|(p, _)| p == k) {
                return Err(bad(format!("{k} is given twice")));
            }
            match k.as_str() {
                "call" => q.call = Some(whole(k, v)?),
                other => return Err(bad(format!("no such parameter {other:?} (call)"))),
            }
        }
        Ok(q)
    }
}

/// The turn number of the path `/v1/turns/{n}/context`: a whole number (one
/// too big for 64 bits is the biggest, which no record has).
pub fn parse_turn(segment: &str) -> Door<u64> {
    whole("turn", segment)
}

fn whole(name: &str, v: &str) -> Door<u64> {
    if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad(format!("{name} must be a whole number, not {v:?}")));
    }
    Ok(v.parse().unwrap_or(u64::MAX))
}

fn bad(message: impl Into<String>) -> DoorError {
    DoorError::new(400, "bad_request", message)
}

/// The turn segment of the path `/v1/turns/{n}/context`, or `None` for any
/// other path.
pub fn turn_of_path(path: &str) -> Option<&str> {
    let n = path.strip_prefix("/v1/turns/")?.strip_suffix("/context")?;
    (!n.is_empty() && !n.contains('/')).then_some(n)
}

// ─── the first pass: sizes and places ────────────────────────────────────────

/// The bands, in the order the model sees them.
const ORDER: [&str; 7] = [
    "tools",
    "system",
    "epoch",
    "history",
    "message",
    "recall",
    "this_turn",
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mark {
    Cached,
    Partly,
    New,
    Unknown,
}

impl Mark {
    fn word(self) -> &'static str {
        match self {
            Mark::Cached => "cached",
            Mark::Partly => "partly",
            Mark::New => "new",
            Mark::Unknown => "unknown",
        }
    }
}

/// A `pack.layers` line seen: where it is and how big its parts are.
struct LayersSeen {
    seq: u64,
    s_hash: String,
    tools_bytes: usize,
    system_bytes: usize,
}

/// An `epoch.rollover` line seen.
struct RollSeen {
    seq: u64,
    l_hash: String,
    /// Bytes of the epoch message.
    slow_bytes: usize,
}

/// A past turn of the epoch: where its `turn.log` is, how big.
struct HistSeen {
    seq: u64,
    bytes: usize,
}

/// This turn's `turn.log`.
struct LogSeen {
    seq: u64,
    /// The line is from after the request was recorded (it has `recall_text`,
    /// null when the turn had no recall).
    sent: bool,
    message_bytes: usize,
    recall_bytes: usize,
    has_recall: bool,
    /// The stored recall text matches its recorded hash.
    recall_intact: bool,
    /// What the record cannot give back whole, in words.
    problems: Vec<String>,
    /// Canonical bytes of each message of the turn.
    sizes: Vec<usize>,
    /// Which of them are the model's.
    assistant: Vec<usize>,
}

struct Plan {
    dir: std::path::PathBuf,
    turn: u64,
    call: u64,
    calls: u64,
    call_line: Map<String, Value>,
    started: Option<Map<String, Value>>,
    cache_event: Option<Value>,
    rewritten: bool,
    layers: Option<u64>,
    roll: Option<u64>,
    hist: Vec<u64>,
    log: LogSeen,
    /// How many of the turn's messages the request held at this call.
    split: usize,
    missing: Vec<&'static str>,
    problems: Vec<String>,
    /// Estimated tokens and cache mark of each band present, in order.
    bands: Vec<(&'static str, usize, Mark)>,
    scale: Option<f64>,
    boundaries: Vec<Value>,
    estimate: usize,
    damaged: Option<Damaged>,
    red: Redactor,
    env_unset: Vec<String>,
}

/// Canonical bytes of one user message of `text`, and its line break.
fn user_bytes(text: &str) -> usize {
    canon::of(&ChatMessage::user(text)).len() + 1
}

/// The page ready to be written: everything that can be refused has been, so
/// [`ContextPage::write_json`] can start an answer.
pub struct ContextPage {
    plan: Plan,
}

/// Open the door for instance `id` of a registry folder: the registry's
/// refusals first, then the record's.
pub fn open(
    registry_dir: &Path,
    id: &str,
    turn: u64,
    q: &ContextQuery,
    base: &Redactor,
) -> Door<ContextPage> {
    let (red, env_unset) = read::fleet_redactor(registry_dir, base);
    read::locate(registry_dir, id)
        .and_then(|(_, entry)| open_dir(&entry.state_dir, turn, q, &red, env_unset))
        .map_err(|e| e.redacted(&red))
}

/// Open the door over the record in `state_dir`. Every refusal (no such turn
/// or call, an unreadable record) is made here, before the first byte.
pub fn open_dir(
    state_dir: &Path,
    turn: u64,
    q: &ContextQuery,
    red: &Redactor,
    env_unset: Vec<String>,
) -> Door<ContextPage> {
    plan(state_dir, turn, q, red, env_unset)
        .map(|plan| ContextPage { plan })
        .map_err(|e| e.redacted(red))
}

fn plan(
    state_dir: &Path,
    turn: u64,
    q: &ContextQuery,
    red: &Redactor,
    env_unset: Vec<String>,
) -> Door<Plan> {
    let mut layers: Vec<LayersSeen> = Vec::new();
    let mut rolls: Vec<RollSeen> = Vec::new();
    let mut hist: Vec<HistSeen> = Vec::new();
    let mut log: Option<LogSeen> = None;
    let mut started: Option<Map<String, Value>> = None;
    let mut calls: Vec<(u64, Map<String, Value>)> = Vec::new();
    let mut events: Vec<(u64, String, String)> = Vec::new();
    let mut rewritten = false;
    let out = read::scan_while(state_dir, |l| {
        match l.kind.as_str() {
            "pack.layers" => {
                if let (Some(h), Some(t), Some(s)) = (
                    l.get("s_hash").as_str(),
                    l.get("tools").as_array(),
                    l.get("system").as_str(),
                ) {
                    layers.push(LayersSeen {
                        seq: l.seq,
                        s_hash: h.to_string(),
                        tools_bytes: canon::of(t).len() + 1,
                        system_bytes: canon::of(s).len(),
                    });
                }
            }
            "epoch.rollover" => {
                // a new epoch ends the history of the one before it
                hist.clear();
                if let (Some(h), Some(s)) = (l.get("l_hash").as_str(), l.get("slow").as_str()) {
                    rolls.push(RollSeen {
                        seq: l.seq,
                        l_hash: h.to_string(),
                        slow_bytes: user_bytes(s),
                    });
                }
            }
            "turn.started" if l.u64("turn") == turn => started = Some(l.body.clone()),
            "llm.call" if l.u64("turn") == turn => calls.push((l.u64("call"), l.body.clone())),
            "cache.break" | "cache.cold" if l.u64("turn") == turn => {
                events.push((l.u64("call"), l.kind.clone(), l.str("cause").to_string()));
            }
            "turn.log" if l.u64("turn") < turn => hist.push(HistSeen {
                seq: l.seq,
                bytes: history_bytes(l),
            }),
            "turn.log" if l.u64("turn") == turn => log = Some(log_seen(l)),
            "turn.ended" if l.u64("turn") == turn => {
                rewritten = l.get("rewritten") == &Value::Bool(true);
                return false;
            }
            _ => {}
        }
        true
    })?;
    let damaged = out.scanned.damaged;
    let hint = damaged
        .as_ref()
        .map(|d| format!(" (the record is damaged after line {})", d.after_seq))
        .unwrap_or_default();
    if started.is_none() && log.is_none() && calls.is_empty() {
        return Err(DoorError::new(
            404,
            "no_such_turn",
            format!("the record has no turn {turn}{hint}"),
        ));
    }
    let Some(log) = log else {
        return Err(DoorError::new(
            409,
            "turn_not_logged",
            format!("turn {turn} has not ended: its messages are written when it does"),
        ));
    };
    let call = q.call.unwrap_or(1);
    let Some((_, line)) = calls.iter().find(|(c, _)| *c == call) else {
        let n = calls.len();
        return Err(DoorError::new(
            404,
            "no_such_call",
            if n == 0 {
                format!("turn {turn} made no recorded model call")
            } else {
                format!("turn {turn} has {n} recorded call(s); call {call} is not one of them")
            },
        ));
    };
    let call_line = line.clone();
    // the layers the call carried, by their hashes (the latest of each: the
    // same hash is the same text)
    let prefix = &call_line["prefix"];
    let s_hash = prefix["s_hash"].as_str().unwrap_or("");
    let l_hash = prefix["l_hash"].as_str().unwrap_or("");
    let layer = layers.iter().rfind(|x| x.s_hash == s_hash);
    let roll = rolls.iter().rfind(|x| x.l_hash == l_hash);
    let mut missing = Vec::new();
    if layer.is_none() {
        missing.extend(["tools", "system"]);
    }
    if roll.is_none() {
        missing.push("epoch");
    }
    if !log.sent {
        missing.push("recall");
    }
    // the request held the turn's messages before the call's own answer
    let split = log
        .assistant
        .get((call - 1) as usize)
        .copied()
        .unwrap_or(log.sizes.len());
    let mut sizes: Vec<(&'static str, usize)> = Vec::new();
    if let Some(x) = layer {
        sizes.push(("tools", x.tools_bytes));
        sizes.push(("system", x.system_bytes));
    }
    if let Some(x) = roll {
        sizes.push(("epoch", x.slow_bytes));
    }
    sizes.push(("history", hist.iter().map(|h| h.bytes).sum()));
    sizes.push(("message", log.message_bytes));
    if log.has_recall {
        sizes.push(("recall", log.recall_bytes));
    }
    sizes.push(("this_turn", log.sizes[..split].iter().sum()));
    // the host's estimate: a token for four bytes, by the pack's own rule
    let est: Vec<(&'static str, usize)> =
        sizes.iter().map(|(b, n)| (*b, canon::tokens(*n))).collect();
    let estimate: usize = est.iter().map(|(_, t)| *t).sum();
    let prompt = call_line["prompt_tokens"].as_u64().unwrap_or(0);
    let cached = call_line["cached_tokens"].as_u64().unwrap_or(0);
    let scale = (prompt > 0 && estimate > 0).then(|| prompt as f64 / estimate as f64);
    // the marks: the provider's cached count against the bands in wire order,
    // the estimates scaled to sum to the provider's own count
    let mut bands = Vec::new();
    let mut ends: Vec<(&'static str, usize)> = Vec::new();
    let mut at = 0usize;
    for (b, t) in &est {
        let (start, end) = (at, at + t);
        at = end;
        ends.push((*b, end));
        let mark = match scale {
            None => Mark::Unknown,
            Some(s) => {
                let (lo, hi) = (start as f64 * s, end as f64 * s);
                if cached == 0 || lo >= cached as f64 {
                    Mark::New
                } else if hi <= cached as f64 {
                    Mark::Cached
                } else {
                    Mark::Partly
                }
            }
        };
        bands.push((*b, *t, mark));
    }
    debug_assert!(
        bands.iter().map(|b| b.0).eq(ORDER
            .iter()
            .copied()
            .filter(|o| bands.iter().any(|b| b.0 == *o)))
    );
    // the two breakpoints the engine sets: the end of the stable layer and of
    // the epoch layer; known where the bands before them are
    let mut boundaries = Vec::new();
    for (after, why, known) in [
        (
            "system",
            "the end of the stable layer: tools and system text",
            layer.is_some(),
        ),
        (
            "epoch",
            "the end of the epoch layer",
            layer.is_some() && roll.is_some(),
        ),
    ] {
        let end = ends.iter().find(|(b, _)| *b == after).map(|(_, e)| *e);
        if let (true, Some(e)) = (known, end) {
            boundaries.push(json!({"after": after, "why": why, "est_tokens": e,
                                   "tokens": scale.map(|s| (e as f64 * s).round() as u64)}));
        }
    }
    let cache_event = events
        .iter()
        .find(|(c, _, _)| *c == call)
        .map(|(_, k, cause)| json!({"kind": k, "cause": cause}));
    Ok(Plan {
        dir: state_dir.to_path_buf(),
        turn,
        call,
        calls: calls.len() as u64,
        call_line,
        started,
        cache_event,
        rewritten,
        layers: layer.map(|x| x.seq),
        roll: roll.map(|x| x.seq),
        hist: hist.iter().map(|h| h.seq).collect(),
        problems: log.problems.clone(),
        log,
        split,
        missing,
        bands,
        scale,
        boundaries,
        estimate,
        damaged,
        red: red.clone(),
        env_unset,
    })
}

/// The header as the model saw it, rebuilt from the one copy of the recall:
/// the header, a line break and the recall block, when the turn had one. And
/// whether that is the whole of it (a turn that was shown a recall the record
/// holds no text for is not).
fn header_as_sent(l: &Line) -> (String, bool) {
    let header = l.str("header");
    match l.get("recall_text").as_str() {
        Some(r) => (format!("{header}\n{r}"), true),
        None => (header.to_string(), l.get("recall") != &Value::Bool(true)),
    }
}

/// A past turn's size in the history: its header as the model saw it and its
/// messages.
fn history_bytes(l: &Line) -> usize {
    user_bytes(&header_as_sent(l).0)
        + messages_of(l)
            .iter()
            .map(|m| canon::bytes(m).len() + 1)
            .sum::<usize>()
}

fn messages_of(l: &Line) -> &[Value] {
    l.get("messages")
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn log_seen(l: &Line) -> LogSeen {
    let messages = messages_of(l);
    let recall = l.get("recall_text").as_str();
    let mut problems = Vec::new();
    let mut recall_intact = true;
    let turn = l.u64("turn");
    match (recall, l.get("recall") == &Value::Bool(true)) {
        (None, true) if l.body.contains_key("recall_text") => problems.push(format!(
            "turn {turn} was shown a recall but the record holds no recall text for it"
        )),
        (Some(r), _) => {
            if let Some(h) = l.get("recall_hash").as_str()
                && h != canon::hash(r.as_bytes())
            {
                recall_intact = false;
                problems.push(format!(
                    "the recall text of turn {turn} does not match its recorded hash; it is shown as recorded"
                ));
            }
        }
        _ => {}
    }
    let (full, _) = header_as_sent(l);
    // the header is one message; its recall is the tail, after a line break
    // (the text's bytes as JSON carries them, without the quotes)
    let recall_bytes = recall.map_or(0, |r| canon::of(r).len().saturating_sub(2));
    let message_bytes = user_bytes(&full).saturating_sub(recall_bytes);
    LogSeen {
        seq: l.seq,
        sent: l.body.contains_key("recall_text"),
        message_bytes,
        recall_bytes,
        has_recall: recall.is_some(),
        recall_intact,
        problems,
        sizes: messages.iter().map(|m| canon::bytes(m).len() + 1).collect(),
        assistant: messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m["role"] == "assistant")
            .map(|(i, _)| i)
            .collect(),
    }
}

// ─── the second pass: the answer, streamed ───────────────────────────────────

/// A record line the answer is made of.
enum Need {
    Layers,
    Roll,
    Hist,
    Log,
}

impl ContextPage {
    /// Write the answer as one JSON object: the small facts first, then the
    /// bands in order, each written as its record line is read (redacted, one
    /// line in memory), then the trailer.
    pub fn write_json(self, out: &mut dyn Write) -> io::Result<()> {
        let p = &self.plan;
        let red = &p.red;
        let c = &p.call_line;
        let started = p.started.as_ref();
        let info = red.redact_value(&json!({
            "turn": p.turn, "call": p.call, "calls": p.calls,
            "epoch": c.get("epoch"),
            "turn_kind": c.get("turn_kind").or_else(|| started.and_then(|s| s.get("turn_kind"))),
            "model": {"requested": c.get("model_requested"), "served": c.get("model_served"),
                      "provider": c.get("provider")},
            "provider": {"prompt_tokens": c.get("prompt_tokens"),
                         "cached_tokens": c.get("cached_tokens"),
                         "cache_write_tokens": c.get("cache_write_tokens"),
                         "completion_tokens": c.get("completion_tokens"),
                         "expected_cached_tokens": c["prefix"].get("expected_cached_tokens")},
            "cache_event": p.cache_event,
            "rewritten": p.rewritten,
            "recorded": p.missing.is_empty() && p.problems.is_empty(),
            "note": if p.missing.is_empty() { Value::Null } else { PREDATES.into() },
            "missing": p.missing,
            "problems": p.problems,
            "estimate_tokens": p.estimate,
            "scale": p.scale.map(canon::fixed),
            "boundaries": p.boundaries,
        }));
        // the fields so far, as an object left open for `bands`
        let head = serde_json::to_string(&info)?;
        out.write_all(&head.as_bytes()[..head.len() - 1])?;
        out.write_all(b",\"bands\":[")?;
        let mut w = Writer {
            out,
            plan: p,
            written: 0,
            rows: 0,
            hist_open: false,
            hist_done: false,
        };
        // the lines read, in the order of the bands
        let mut need: Vec<(u64, Need)> = Vec::new();
        if let Some(s) = p.layers {
            need.push((s, Need::Layers));
        }
        if let Some(s) = p.roll {
            need.push((s, Need::Roll));
        }
        need.extend(p.hist.iter().map(|s| (*s, Need::Hist)));
        need.push((p.log.seq, Need::Log));
        // normally one pass reads them all; a layer written after the epoch
        // layer (a swap in the middle of an epoch) is behind the read, and the
        // next pass reads from the start again
        let mut at = 0usize;
        while at < need.len() {
            let before = at;
            let mut failed: Option<io::Error> = None;
            read::scan_while(&p.dir, |l| {
                let (seq, kind) = &need[at];
                if l.seq > *seq {
                    return false;
                }
                if l.seq == *seq {
                    if let Err(e) = w.line(kind, l) {
                        failed = Some(e);
                        return false;
                    }
                    at += 1;
                }
                at < need.len()
            })
            .map_err(|e| io::Error::other(e.message))?;
            if let Some(e) = failed {
                return Err(e);
            }
            if at == before {
                return Err(io::Error::other(
                    "the record changed under the read: a line this answer needs is gone",
                ));
            }
        }
        let trailer = red.redact_value(&json!({
            "damaged": p.damaged.as_ref().map(Damaged::to_json),
            "redaction": {"env_unset": p.env_unset},
        }));
        let text = serde_json::to_string(&trailer)?;
        out.write_all(b"],")?;
        out.write_all(&text.as_bytes()[1..])?;
        out.flush()
    }
}

struct Writer<'a> {
    out: &'a mut dyn Write,
    plan: &'a Plan,
    /// Bands written.
    written: usize,
    /// Rows of the history band written.
    rows: usize,
    hist_open: bool,
    hist_done: bool,
}

impl Writer<'_> {
    fn info(&self, band: &str) -> (usize, &'static str) {
        self.plan
            .bands
            .iter()
            .find(|(b, _, _)| *b == band)
            .map(|(_, t, m)| (*t, m.word()))
            .unwrap_or((0, "unknown"))
    }

    fn comma(&mut self) -> io::Result<()> {
        if self.written > 0 {
            self.out.write_all(b",")?;
        }
        self.written += 1;
        Ok(())
    }

    /// One whole band, redacted.
    fn band(&mut self, band: &str, mut body: Map<String, Value>) -> io::Result<()> {
        let (est, mark) = self.info(band);
        body.insert("band".into(), band.into());
        body.insert("est_tokens".into(), est.into());
        body.insert("cache".into(), mark.into());
        let v = self.plan.red.redact_value(&Value::Object(body));
        self.comma()?;
        serde_json::to_writer(&mut *self.out, &v)?;
        Ok(())
    }

    fn history_open(&mut self) -> io::Result<()> {
        if self.hist_open {
            return Ok(());
        }
        self.hist_open = true;
        let (est, mark) = self.info("history");
        self.comma()?;
        write!(
            self.out,
            "{{\"band\":\"history\",\"cache\":\"{mark}\",\"est_tokens\":{est},\"turns\":{},\"rows\":[",
            self.plan.hist.len()
        )
    }

    fn history_close(&mut self) -> io::Result<()> {
        self.history_open()?;
        if !self.hist_done {
            self.hist_done = true;
            self.out.write_all(b"]}")?;
        }
        Ok(())
    }

    fn line(&mut self, need: &Need, l: &Line) -> io::Result<()> {
        match need {
            Need::Layers => {
                let items: Vec<Value> = l
                    .get("tools")
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                    .iter()
                    .map(|t| json!({"name": t["name"], "text": canon::string(t)}))
                    .collect();
                let mut m = Map::new();
                m.insert("count".into(), items.len().into());
                m.insert("items".into(), Value::Array(items));
                self.band("tools", m)?;
                let mut m = Map::new();
                m.insert("text".into(), l.get("system").clone());
                self.band("system", m)
            }
            Need::Roll => {
                let mut m = Map::new();
                m.insert("text".into(), l.get("slow").clone());
                self.band("epoch", m)
            }
            Need::Hist => {
                self.history_open()?;
                let (header, complete) = header_as_sent(l);
                let row = self.plan.red.redact_value(&json!({
                    "turn": l.u64("turn"),
                    "header": header,
                    "header_complete": complete,
                    "messages": l.get("messages"),
                }));
                if self.rows > 0 {
                    self.out.write_all(b",")?;
                }
                self.rows += 1;
                serde_json::to_writer(&mut *self.out, &row)?;
                Ok(())
            }
            Need::Log => {
                self.history_close()?;
                let mut m = Map::new();
                m.insert("text".into(), l.get("header").clone());
                self.band("message", m)?;
                if let Some(r) = l.get("recall_text").as_str() {
                    let mut m = Map::new();
                    m.insert("text".into(), r.into());
                    m.insert("intact".into(), self.plan.log.recall_intact.into());
                    self.band("recall", m)?;
                }
                let upto = self.plan.split.min(messages_of(l).len());
                let mut m = Map::new();
                m.insert("calls_so_far".into(), (self.plan.call - 1).into());
                m.insert(
                    "messages".into(),
                    Value::Array(messages_of(l)[..upto].to_vec()),
                );
                self.band("this_turn", m)
            }
        }
    }
}
