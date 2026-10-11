//! The read doors' library (design note H3): what a gateway answers about a
//! native instance from its own files, with the host not running or running.
//!
//! Three reads, all computed from the registry entry and the instance's state
//! directory and never writing to either:
//!
//! - [`rows`]: every registered instance with its state word (the registry's:
//!   the state lock and the record's last line), the command that starts it, and
//!   the time of its last event;
//! - [`summary`]: the instance's **fold**: the record replayed through the
//!   host's own [`State::apply`], read as what it is doing, what it waits for,
//!   its queue head, its boundary, its desk's limits and its config as read;
//! - [`record()`]: the recorded events, paged by the caller and replayable.
//!
//! A stopped instance reads the same as a running one: nothing here asks the
//! host anything. Every answer is passed through the shared redactor
//! ([`crate::redact`]) before it is returned; the record on disk stays
//! verbatim. A record that is damaged is read as far as it is intact and says
//! where it stops ([`Damaged`]); a missing or unreadable one is a [`DoorError`],
//! never a panic. Door shapes: `docs/rung-host-api.md` (H3).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::clock::{HOUR, iso};
use crate::desk::{Knobs, SpendCap};
use crate::kernel::COPY_LOOP_TURNS;
use crate::record::{self, Line};
use crate::redact::Redactor;
use crate::registry::{self, Entry, Row, Word};
use crate::state::State;

/// The page a record read gives when the caller names no size. A default,
/// not a bound: `limit` has no maximum.
pub const DEFAULT_LIMIT: u64 = 500;

/// Why a door cannot answer: an HTTP status, a stable code and a sentence.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl DoorError {
    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, "bad_request", message)
    }

    fn unreadable(what: &Path, e: &std::io::Error) -> Self {
        Self::new(500, "unreadable", format!("{}: {e}", what.display()))
    }
}

impl std::fmt::Display for DoorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for DoorError {}

pub type Door<T = Value> = Result<T, DoorError>;

// ─── scanning the record ─────────────────────────────────────────────────────

/// Where a record stops being intact.
#[derive(Debug, Clone, PartialEq)]
pub struct Damaged {
    /// The last line number that was good (every line before the damage).
    pub after_seq: u64,
    /// The segment file holding the bad line.
    pub segment: String,
    /// The bad line's number in that file, counting from 1.
    pub line: u64,
    pub problem: String,
}

impl Damaged {
    fn to_json(&self) -> Value {
        json!({"after_seq": self.after_seq, "segment": self.segment,
               "line": self.line, "problem": self.problem})
    }
}

/// What a scan saw: the lines of the intact prefix, and where it ended.
#[derive(Debug, Clone, PartialEq)]
pub struct Scanned {
    pub total: u64,
    pub damaged: Option<Damaged>,
}

/// The state directory must be there: a dropped one is an error naming it,
/// not an empty instance.
fn check_state_dir(state_dir: &Path) -> Door<()> {
    match std::fs::metadata(state_dir) {
        Ok(m) if m.is_dir() => Ok(()),
        Ok(_) => Err(DoorError::new(
            500,
            "unreadable",
            format!("state directory {} is not a directory", state_dir.display()),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(DoorError::new(
            404,
            "state_dir_missing",
            format!("state directory {} does not exist", state_dir.display()),
        )),
        Err(e) => Err(DoorError::unreadable(state_dir, &e)),
    }
}

/// Read the record in `state_dir` line by line, calling `each` on every
/// line of its intact prefix, in order. Lines are numbered from 1 with no gap,
/// as the host writes them; the first line that is not a record line, is not
/// the next number, or is not UTF-8 ends the prefix and is reported. A torn
/// last line of the newest segment is a write in flight, not damage. A record
/// directory that is not there is an empty record. Memory is one line.
pub fn scan(state_dir: &Path, mut each: impl FnMut(&Line)) -> Door<Scanned> {
    check_state_dir(state_dir)?;
    let dir = state_dir.join("record");
    let segs = record::segments(&dir).map_err(|e| DoorError::unreadable(&dir, &e))?;
    let mut index = 0u64;
    let mut buf = Vec::new();
    for (si, (_, path)) in segs.iter().enumerate() {
        let newest = si + 1 == segs.len();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut r = BufReader::new(File::open(path).map_err(|e| DoorError::unreadable(path, &e))?);
        let mut lineno = 0u64;
        loop {
            buf.clear();
            let n = r
                .read_until(b'\n', &mut buf)
                .map_err(|e| DoorError::unreadable(path, &e))?;
            if n == 0 {
                break;
            }
            lineno += 1;
            let terminated = buf.last() == Some(&b'\n');
            if !terminated && newest {
                break; // a write in flight, or torn by a crash
            }
            let damage = |problem: String| Scanned {
                total: index,
                damaged: Some(Damaged {
                    after_seq: index,
                    segment: name.clone(),
                    line: lineno,
                    problem,
                }),
            };
            let Ok(text) = std::str::from_utf8(&buf) else {
                return Ok(damage("the line is not UTF-8".into()));
            };
            let text = text.trim_end_matches(['\n', '\r']);
            if text.trim().is_empty() {
                continue;
            }
            let Some(line) = Line::parse(text) else {
                return Ok(damage("the line is not a record line".into()));
            };
            if line.seq != index + 1 {
                return Ok(damage(format!(
                    "expected line number {}, found {}",
                    index + 1,
                    line.seq
                )));
            }
            each(&line);
            index += 1;
        }
    }
    Ok(Scanned {
        total: index,
        damaged: None,
    })
}

// ─── the fold ────────────────────────────────────────────────────────────────

/// The record replayed: the host's [`State`], and what only the lines say.
#[derive(Debug, Clone)]
pub struct Fold {
    pub state: State,
    pub lines: u64,
    /// `{seq, at, kind}` of the last line.
    pub last: Option<Value>,
    /// The body of the latest `host.start` line: the config the host started
    /// with, its desk and its memory.
    pub started: Option<Map<String, Value>>,
    /// `{n, at, mode, pending}` of the latest `boundary` line.
    pub boundary: Option<Value>,
    /// The turn started and not yet ended.
    pub turn_in_progress: Option<Value>,
    /// The `why` of a final `halted` line.
    pub halted_why: Option<String>,
    pub damaged: Option<Damaged>,
}

/// Replay the record in `state_dir` through the host's own fold. Pure and
/// read-only: the same lines give the same state, byte for byte.
pub fn fold(state_dir: &Path) -> Door<Fold> {
    let mut f = Fold {
        state: State::default(),
        lines: 0,
        last: None,
        started: None,
        boundary: None,
        turn_in_progress: None,
        halted_why: None,
        damaged: None,
    };
    let scanned = scan(state_dir, |l| {
        f.state.apply(l);
        f.last = Some(json!({"seq": l.seq, "at": l.at, "kind": l.kind}));
        f.halted_why = None;
        match l.kind.as_str() {
            "host.start" => {
                f.started = Some(l.body.clone());
                f.turn_in_progress = None;
            }
            "recovered" => f.turn_in_progress = None,
            "boundary" => {
                f.boundary = Some(
                    json!({"n": l.get("n"), "at": l.at, "mode": l.get("mode"), "pending": l.get("pending")}),
                );
            }
            "turn.started" => {
                f.turn_in_progress = Some(json!({
                    "turn": l.get("turn"), "turn_kind": l.get("turn_kind"), "started_at": l.at,
                    "model": l.get("model"), "project": l.get("project"), "mode": l.get("mode"),
                }));
            }
            "turn.ended" => {
                if f.turn_in_progress
                    .as_ref()
                    .is_some_and(|t| t["turn"] == *l.get("turn"))
                {
                    f.turn_in_progress = None;
                }
            }
            "halted" => f.halted_why = Some(l.str("why").to_string()),
            _ => {}
        }
    })?;
    f.lines = scanned.total;
    f.damaged = scanned.damaged;
    Ok(f)
}

// ─── the redactor and the config file ────────────────────────────────────────

/// A shell word for `s`: itself when plain, else single-quoted.
fn shell_word(s: &str) -> String {
    let plain = !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&b));
    if plain {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// The command that starts the instance, when its config file is known.
fn start_command(entry: &Entry) -> Option<String> {
    entry.config.as_ref().map(|c| {
        format!(
            "rung-host run --config {}",
            shell_word(&c.to_string_lossy())
        )
    })
}

/// The names of the variables a config file says hold secrets: every
/// `*_env` key, and the values of `acp.tokens`.
fn env_names(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if k.ends_with("_env")
                    && let Some(s) = x.as_str()
                {
                    out.push(s.to_string());
                } else if k == "tokens"
                    && let Some(t) = x.as_object()
                {
                    out.extend(t.values().filter_map(|s| s.as_str().map(str::to_string)));
                } else {
                    env_names(x, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| env_names(x, out)),
        _ => {}
    }
}

/// An instance's config file as read, and the redactor for what it names.
struct ConfigView {
    path: Option<String>,
    content: Option<Value>,
    problem: Option<String>,
}

impl ConfigView {
    fn read(entry: &Entry) -> Self {
        let Some(path) = &entry.config else {
            return Self {
                path: None,
                content: None,
                problem: None,
            };
        };
        let parsed = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_yaml::from_str::<Value>(&t).map_err(|e| e.to_string()));
        let (content, problem) = match parsed {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(e)),
        };
        Self {
            path: Some(path.display().to_string()),
            content,
            problem,
        }
    }

    /// `base` plus the value of every variable this config names.
    fn redactor(&self, base: &Redactor) -> Redactor {
        let mut r = base.clone();
        let mut names = Vec::new();
        if let Some(c) = &self.content {
            env_names(c, &mut names);
        }
        for n in names {
            r.add_env(&n);
        }
        r
    }

    fn to_json(&self) -> Value {
        json!({"path": self.path, "content": self.content, "problem": self.problem})
    }
}

// ─── finding an instance ─────────────────────────────────────────────────────

fn locate(registry_dir: &Path, id: &str) -> Door<(Row, Entry)> {
    let row = registry::row_in(registry_dir, id)
        .map_err(|e| DoorError::new(500, "unreadable", e))?
        .ok_or_else(|| DoorError::new(404, "no_such_instance", "no instance has that id"))?;
    let Some(entry) = row.entry.clone() else {
        return Err(DoorError::new(
            500,
            "unreadable",
            row.problem
                .clone()
                .unwrap_or_else(|| "the registry entry cannot be read".into()),
        ));
    };
    Ok((row, entry))
}

/// Is `id` a native instance the registry folder lists (readable or not)?
pub fn is_registered(registry_dir: &Path, id: &str) -> bool {
    matches!(registry::row_in(registry_dir, id), Ok(Some(_)))
}

// ─── the summary ─────────────────────────────────────────────────────────────

fn state_label(row: &Row, f: &Fold) -> &'static str {
    match row.word {
        Word::Down => return "Down",
        Word::Stopped => return "Stopped",
        Word::Unreadable => return "Unreadable",
        Word::Running => {}
    }
    let gov = &f.state.governor;
    let blocked = gov.blocked || gov.degraded.as_ref().is_some_and(|w| w.class == "blocked");
    if blocked || f.state.kernel.copy_streak >= COPY_LOOP_TURNS {
        "Stuck"
    } else if gov.degraded.is_some() {
        "Waiting"
    } else if !f.state.inbox.in_flight.is_empty()
        || f.turn_in_progress
            .as_ref()
            .is_some_and(|t| t["turn_kind"] == "responding")
    {
        "Answering"
    } else if f.state.kernel.commitment().is_some() {
        "Working"
    } else {
        "Free time"
    }
}

fn doing(label: &str, f: &Fold) -> String {
    match label {
        "Stopped" => match f.halted_why.as_deref() {
            Some(w) if !w.is_empty() => format!("Halted ({w})"),
            _ => "Halted".into(),
        },
        "Down" => match &f.last {
            Some(l) => format!(
                "Not running; last line at {}",
                iso(l["at"].as_i64().unwrap_or(0))
            ),
            None => "Not running; no record".into(),
        },
        "Unreadable" => "Cannot be read".into(),
        _ => match f.state.kernel.commitment() {
            Some(c) => format!("Committed to {}: {}", c.project, c.title),
            None => "Free time".into(),
        },
    }
}

fn waiting_for(label: &str, f: &Fold) -> Value {
    if matches!(label, "Stopped" | "Down" | "Unreadable") {
        return Value::Null;
    }
    let st = &f.state;
    if let Some(w) = &st.governor.degraded {
        return json!({"kind": "degraded", "class": w.class, "until": w.until, "why": w.why});
    }
    if let Some(t) = &f.turn_in_progress {
        return json!({"kind": "turn", "turn": t["turn"], "since": t["started_at"]});
    }
    if !st.inbox.pending.is_empty() {
        return json!({"kind": "boundary", "pending": st.inbox.pending.len()});
    }
    if let Some((e, due)) = st.calendar.next_entry() {
        return json!({"kind": "calendar", "at": due, "id": e.id, "text": e.text});
    }
    Value::Null
}

fn queue_view(f: &Fold) -> Value {
    let st = &f.state.inbox;
    let head = st.waiting().first().map(|p| {
        let mut v = serde_json::to_value(&p.item).unwrap_or(Value::Null);
        v["order"] = json!(p.order);
        v
    });
    json!({"waiting": st.pending.len(), "in_flight": st.in_flight.len(), "head": head})
}

fn desk_view(f: &Fold, cfg: &ConfigView) -> Value {
    let started = f.started.as_ref();
    let desk = &f.state.desk;
    let cap = SpendCap::default();
    let from_file = |k: &str, d: f64| {
        cfg.content
            .as_ref()
            .and_then(|c| c["desk"][k].as_f64())
            .unwrap_or(d)
    };
    let mut limits = match serde_json::to_value(Knobs::default()) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    };
    limits.insert(
        "cap_usd_day".into(),
        json!(from_file("cap_usd_day", cap.per_day)),
    );
    limits.insert(
        "cap_usd_ask".into(),
        json!(from_file("cap_usd_ask", cap.per_ask)),
    );
    let last = f.state.last_at;
    json!({
        "backend": started.map_or(Value::Null, |s| s.get("desk").cloned().unwrap_or(Value::Null)),
        "mode": started.map_or(Value::Null, |s| s.get("desk_mode").cloned().unwrap_or(Value::Null)),
        "spent_today_usd": desk.spent_today,
        "interrupts_last_hour": desk.interrupts.iter().filter(|t| **t > last - HOUR).count(),
        "enabled_groups": desk.groups.iter().filter(|(_, g)| g.enabled).map(|(k, _)| k).collect::<Vec<_>>(),
        "limits": limits,
    })
}

fn governor_view(f: &Fold) -> Value {
    let gov = &f.state.governor;
    let ladder: Vec<Value> = f
        .started
        .as_ref()
        .and_then(|s| s.get("config"))
        .and_then(|c| c["ladder"].as_array())
        .map(|l| {
            l.iter()
                .enumerate()
                .map(|(i, m)| json!({"rung": i, "model": m, "available": !gov.unavailable.contains(&i)}))
                .collect()
        })
        .unwrap_or_default();
    json!({
        "rung": gov.rung,
        "model": ladder.get(gov.rung).map_or(Value::Null, |r| r["model"].clone()),
        "failures": gov.failures,
        "blocked": gov.blocked,
        "ladder": ladder,
    })
}

/// The summary for one located instance, redacted.
fn summary_value(row: &Row, entry: &Entry, base: &Redactor) -> Door {
    let f = fold(&entry.state_dir)?;
    let cfg = ConfigView::read(entry);
    let label = state_label(row, &f);
    let st = &f.state;
    let started_cfg = f.started.as_ref().and_then(|s| s.get("config"));
    let quota = started_cfg.and_then(|c| c["governor"]["quota"]["rpd"].as_u64());
    let problem = f
        .damaged
        .as_ref()
        .map(|d| {
            format!(
                "the record is damaged at {} line {}: {}; this is the fold of the {} lines before it",
                d.segment, d.line, d.problem, d.after_seq
            )
        })
        .or_else(|| row.problem.clone());
    let needs_you = matches!(label, "Stuck" | "Down");
    let v = json!({
        "contract": 1,
        "id": entry.id,
        "name": entry.name,
        "word": row.word.as_str(),
        "state": label,
        "doing": doing(label, &f),
        "needs_you": needs_you,
        "turn": (st.turn > 0).then_some(st.turn),
        "last_seq": f.lines,
        "last_at": (f.lines > 0).then_some(st.last_at),
        "requests": st.governor.requests_today,
        "quota": quota,
        "spend_usd_day": st.governor.paid_spent_today,
        "day": (f.lines > 0).then(|| iso(st.last_at)[..10].to_string()),
        "start_command": start_command(entry),
        "waiting_for": waiting_for(label, &f),
        "boundary": f.boundary,
        "turn_in_progress": f.turn_in_progress,
        "queue": queue_view(&f),
        "desk": desk_view(&f, &cfg),
        "governor": governor_view(&f),
        "config": {"file": cfg.to_json(), "started_with": started_cfg},
        "fold_hash": st.hash(),
        "damaged": f.damaged.as_ref().map(Damaged::to_json),
        "problem": problem,
    });
    Ok(cfg.redactor(base).redact_value(&v))
}

/// `GET /v1/summary`: the fold of instance `id`, read from its own files.
pub fn summary(registry_dir: &Path, id: &str, base: &Redactor) -> Door {
    let (row, entry) = locate(registry_dir, id)?;
    summary_value(&row, &entry, base)
}

// ─── the list ────────────────────────────────────────────────────────────────

/// Every instance in the registry folder, by id, as the list door shows it:
/// the registry's row (the entry's fields, `word`, `last`, `problem`), the
/// time of the last event, the command that starts it and its fold's summary
/// (null, with `error`, when it cannot be read). No row is dropped.
pub fn rows(registry_dir: &Path, base: &Redactor) -> Door<Vec<Value>> {
    let rows = registry::list_in(registry_dir).map_err(|e| DoorError::new(500, "unreadable", e))?;
    Ok(rows
        .iter()
        .map(|row| {
            let mut v = row.to_json();
            let summary = match &row.entry {
                Some(entry) => summary_value(row, entry, base),
                None => Err(DoorError::new(
                    500,
                    "unreadable",
                    row.problem.clone().unwrap_or_default(),
                )),
            };
            let (summary, error) = match summary {
                Ok(s) => (s, Value::Null),
                Err(e) => (Value::Null, json!(e.message)),
            };
            v["name"] = row.entry.as_ref().map_or(json!(row.id), |e| json!(e.name));
            v["source"] = json!("registry");
            v["reachable"] = json!(row.word != Word::Unreadable);
            v["last_at"] = row.last.as_ref().map_or(Value::Null, |l| json!(l.at));
            v["start_command"] = row
                .entry
                .as_ref()
                .and_then(start_command)
                .map_or(Value::Null, Value::from);
            if v["problem"].is_null() && !error.is_null() {
                v["problem"] = error.clone();
            }
            v["summary"] = summary;
            v["error"] = error;
            // the row as a whole is text going out: redact it too
            let cfg = row.entry.as_ref().map(ConfigView::read);
            match cfg {
                Some(c) => c.redactor(base).redact_value(&v),
                None => base.redact_value(&v),
            }
        })
        .collect())
}

// ─── the paged events ────────────────────────────────────────────────────────

/// A query on the record door. No maximum on `limit`.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordQuery {
    /// Lines to skip: from the oldest, or from the newest when `desc`.
    pub offset: u64,
    /// The cursor: lines after this line number (oldest first only).
    pub after: Option<u64>,
    pub limit: u64,
    pub desc: bool,
}

impl Default for RecordQuery {
    fn default() -> Self {
        Self {
            offset: 0,
            after: None,
            limit: DEFAULT_LIMIT,
            desc: false,
        }
    }
}

impl RecordQuery {
    /// Parse the query's decoded pairs. Unknown names, repeated names,
    /// values that are not whole numbers (and a `limit` of 0), `offset` with
    /// `after`, and `after` with `order=desc` are 400. A number too big for 64
    /// bits is the biggest one: a page size has no maximum.
    pub fn parse(pairs: &[(String, String)]) -> Door<Self> {
        let mut q = Self::default();
        let mut seen: Vec<&str> = Vec::new();
        let mut has_offset = false;
        for (k, v) in pairs {
            if seen.contains(&k.as_str()) {
                return Err(DoorError::bad_request(format!("{k} is given twice")));
            }
            seen.push(k);
            let number = || -> Door<u64> {
                if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(DoorError::bad_request(format!(
                        "{k} must be a whole number, not {v:?}"
                    )));
                }
                Ok(v.parse().unwrap_or(u64::MAX))
            };
            match k.as_str() {
                "offset" => {
                    q.offset = number()?;
                    has_offset = true;
                }
                "after" => q.after = Some(number()?),
                "limit" => {
                    q.limit = number()?;
                    if q.limit == 0 {
                        return Err(DoorError::bad_request("limit must be at least 1"));
                    }
                }
                "order" => match v.as_str() {
                    "asc" => q.desc = false,
                    "desc" => q.desc = true,
                    _ => return Err(DoorError::bad_request("order is asc or desc")),
                },
                other => {
                    return Err(DoorError::bad_request(format!(
                        "no such parameter {other:?} (offset, after, limit, order)"
                    )));
                }
            }
        }
        if has_offset && q.after.is_some() {
            return Err(DoorError::bad_request("give offset or after, not both"));
        }
        if q.desc && q.after.is_some() {
            return Err(DoorError::bad_request(
                "after pages oldest first; it cannot go with order=desc",
            ));
        }
        Ok(q)
    }
}

/// `GET /v1/record`: the recorded events of instance `id`, one page. Oldest
/// first by default. The page is the caller's: `limit` lines from `offset` (or
/// after line number `after`); `total` is the number of intact lines; `next`
/// is the next offset or null on the last page; `cursor` is the `after` that
/// continues from this page. A cursor past the end is an empty last page.
pub fn record(registry_dir: &Path, id: &str, q: &RecordQuery, base: &Redactor) -> Door {
    let (_, entry) = locate(registry_dir, id)?;
    let cfg = ConfigView::read(&entry);
    let dir = &entry.state_dir;
    let offset = q.after.unwrap_or(q.offset);
    let mut lines: Vec<Value> = Vec::new();
    let (total, damaged) = if q.desc {
        let counted = scan(dir, |_| {})?;
        let hi = counted.total.saturating_sub(offset);
        let lo = hi.saturating_sub(q.limit);
        let mut at = 0u64;
        let s = scan(dir, |l| {
            if at >= lo && at < hi {
                lines.push(l.to_value());
            }
            at += 1;
        })?;
        lines.reverse();
        (counted.total.max(s.total), counted.damaged.or(s.damaged))
    } else {
        let end = offset.saturating_add(q.limit);
        let mut at = 0u64;
        let s = scan(dir, |l| {
            if at >= offset && at < end {
                lines.push(l.to_value());
            }
            at += 1;
        })?;
        (s.total, s.damaged)
    };
    let end = offset.saturating_add(lines.len() as u64);
    let next = (end < total).then_some(end);
    let v = json!({
        "lines": lines,
        "offset": offset,
        "limit": q.limit,
        "total": total,
        "next": next,
        "cursor": (!q.desc).then_some(end.max(offset)),
        "order": if q.desc { "desc" } else { "asc" },
        "after": q.after,
        "damaged": damaged.as_ref().map(Damaged::to_json),
    });
    Ok(cfg.redactor(base).redact_value(&v))
}
