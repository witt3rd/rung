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
//! - [`record_open`] and [`RecordPage::write_json`]: the recorded events, paged by
//!   the caller, replayable, and written a line at a time.
//!
//! A stopped instance reads the same as a running one: nothing here asks the
//! host anything. Every answer is passed through the shared redactor
//! ([`crate::redact`]) before it is returned; the record on disk stays
//! verbatim. A record that is damaged is read as far as it is intact and says
//! where it stops ([`Damaged`]); a missing or unreadable one is a [`DoorError`],
//! never a panic. Door shapes: `docs/rung-host-api.md` (H3).

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
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
    pub(crate) fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
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
    pub(crate) fn to_json(&self) -> Value {
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
pub(crate) fn check_state_dir(state_dir: &Path) -> Door<()> {
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

/// Where the intact prefix ends, for reading it backwards: the segment's index
/// in the list and the byte offset just after the last good line.
#[derive(Debug, Clone, Copy)]
struct End {
    segment: usize,
    bytes: u64,
}

#[derive(Debug)]
pub(crate) struct ScanOut {
    pub(crate) scanned: Scanned,
    segments: Vec<std::path::PathBuf>,
    end: Option<End>,
}

/// Read the record in `state_dir` line by line, calling `each` on every
/// line of its intact prefix, in order. Lines are numbered from 1 with no gap,
/// as the host writes them; the first line that is not a record line, is not
/// the next number, or is not UTF-8 ends the prefix and is reported. A torn
/// last line of the newest segment is a write in flight, not damage. A record
/// directory that is not there is an empty record. Memory is one line.
pub fn scan(state_dir: &Path, mut each: impl FnMut(&Line)) -> Door<Scanned> {
    scan_while(state_dir, |l| {
        each(l);
        true
    })
    .map(|o| o.scanned)
}

/// [`scan`], stopping early (the `total` is then only what was read) when
/// `each` returns false.
pub(crate) fn scan_while(state_dir: &Path, mut each: impl FnMut(&Line) -> bool) -> Door<ScanOut> {
    check_state_dir(state_dir)?;
    let dir = state_dir.join("record");
    let segs: Vec<std::path::PathBuf> = record::segments(&dir)
        .map_err(|e| DoorError::unreadable(&dir, &e))?
        .into_iter()
        .map(|(_, p)| p)
        .collect();
    let mut index = 0u64;
    let mut end = None;
    let mut buf = Vec::new();
    for (si, path) in segs.iter().enumerate() {
        let newest = si + 1 == segs.len();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut r = BufReader::new(File::open(path).map_err(|e| DoorError::unreadable(path, &e))?);
        let mut lineno = 0u64;
        let mut pos = 0u64;
        loop {
            buf.clear();
            let n = r
                .read_until(b'\n', &mut buf)
                .map_err(|e| DoorError::unreadable(path, &e))?;
            if n == 0 {
                break;
            }
            lineno += 1;
            pos += n as u64;
            let terminated = buf.last() == Some(&b'\n');
            if !terminated && newest {
                break; // a write in flight, or torn by a crash
            }
            let damage = |problem: String| ScanOut {
                scanned: Scanned {
                    total: index,
                    damaged: Some(Damaged {
                        after_seq: index,
                        segment: name.clone(),
                        line: lineno,
                        problem,
                    }),
                },
                segments: segs.clone(),
                end,
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
            index += 1;
            end = Some(End {
                segment: si,
                bytes: pos,
            });
            if !each(&line) {
                return Ok(ScanOut {
                    scanned: Scanned {
                        total: index,
                        damaged: None,
                    },
                    segments: segs,
                    end,
                });
            }
        }
    }
    Ok(ScanOut {
        scanned: Scanned {
            total: index,
            damaged: None,
        },
        segments: segs,
        end,
    })
}

// ─── the record index ────────────────────────────────────────────────────────

/// How many intact lines lie between two seek marks. A unit of work (a page
/// reads at most this many lines it does not return), not a bound on any text.
const MARK_EVERY: u64 = 1024;

/// A place in the record where line number `index` (counting from 0) begins:
/// the segment's position in the list and the byte offset in it.
#[derive(Debug, Clone, Copy)]
struct Mark {
    segment: usize,
    bytes: u64,
    index: u64,
}

/// What one pass over a record learned, kept so the next page reads only what
/// was appended since: the intact prefix (its length, where it ends, the first
/// damage) and a seek mark every [`MARK_EVERY`] lines. A page then costs the
/// lines it returns plus at most one stride, not the whole record. The record
/// is append-only, so a sealed segment's result never changes; a segment that
/// shrank or vanished, or a segment list that is not an extension of the one
/// scanned, drops the index and it is built again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ident {
    ino: u64,
    len: u64,
    mtime: (i64, i64),
}

impl Ident {
    fn of(path: &Path) -> Option<Ident> {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::metadata(path).ok()?;
        Some(Ident {
            ino: m.ino(),
            len: m.len(),
            mtime: (m.mtime(), m.mtime_nsec()),
        })
    }
}

thread_local! {
    static LINES_READ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn count_line() {
    LINES_READ.with(|c| c.set(c.get() + 1));
}

/// Lines the record door has read on this thread (a measure of work, for a test).
#[doc(hidden)]
pub fn record_lines_read() -> u64 {
    LINES_READ.with(|c| c.get())
}

#[derive(Debug, Default)]
struct RecIdx {
    segments: Vec<std::path::PathBuf>,
    /// The identity of each segment the scan has finished, by position.
    done: Vec<Ident>,
    /// The inode of the segment being scanned.
    cur_ino: Option<u64>,
    /// Where the next scan resumes: segment, bytes, and lines already read in it.
    seg: usize,
    pos: u64,
    lineno: u64,
    total: u64,
    damaged: Option<Damaged>,
    end: Option<End>,
    marks: Vec<Mark>,
}

impl RecIdx {
    /// Is what was scanned still the front of what is on disk?
    fn still_valid(&self, now: &[std::path::PathBuf]) -> bool {
        if now.len() < self.segments.len() || now[..self.segments.len()] != self.segments[..] {
            return false;
        }
        for (i, id) in self.done.iter().enumerate() {
            if Ident::of(&self.segments[i]) != Some(*id) {
                return false;
            }
        }
        if self.seg < self.segments.len() {
            match Ident::of(&self.segments[self.seg]) {
                Some(id) if id.len >= self.pos && self.cur_ino.is_none_or(|i| i == id.ino) => {}
                _ => return false,
            }
        }
        true
    }

    /// Read what was appended since the last pass.
    fn advance(&mut self, now: Vec<std::path::PathBuf>) -> Door<()> {
        if !self.still_valid(&now) {
            *self = RecIdx::default();
        }
        if self.marks.is_empty() {
            self.marks.push(Mark {
                segment: 0,
                bytes: 0,
                index: 0,
            });
        }
        self.segments = now;
        if self.damaged.is_some() {
            return Ok(());
        }
        let mut buf = Vec::new();
        while self.seg < self.segments.len() {
            let si = self.seg;
            let path = self.segments[si].clone();
            let newest = si + 1 == self.segments.len();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut f = File::open(&path).map_err(|e| DoorError::unreadable(&path, &e))?;
            {
                use std::os::unix::fs::MetadataExt;
                let ino = f
                    .metadata()
                    .map_err(|e| DoorError::unreadable(&path, &e))?
                    .ino();
                if self.cur_ino.is_some_and(|i| i != ino) {
                    return Err(DoorError::unreadable(
                        &path,
                        &io::Error::other("the segment was replaced while it was read"),
                    ));
                }
                self.cur_ino = Some(ino);
            }
            f.seek(SeekFrom::Start(self.pos))
                .map_err(|e| DoorError::unreadable(&path, &e))?;
            let mut r = BufReader::new(f);
            loop {
                buf.clear();
                let n = r
                    .read_until(b'\n', &mut buf)
                    .map_err(|e| DoorError::unreadable(&path, &e))?;
                if n == 0 {
                    break;
                }
                count_line();
                let terminated = buf.last() == Some(&b'\n');
                if !terminated && newest {
                    return Ok(()); // a write in flight, or torn by a crash
                }
                let lineno = self.lineno + 1;
                let mut damage = |problem: String| {
                    self.damaged = Some(Damaged {
                        after_seq: self.total,
                        segment: name.clone(),
                        line: lineno,
                        problem,
                    });
                };
                let Ok(text) = std::str::from_utf8(&buf) else {
                    damage("the line is not UTF-8".into());
                    return Ok(());
                };
                let text = text.trim_end_matches(['\n', '\r']);
                if text.trim().is_empty() {
                    self.pos += n as u64;
                    self.lineno = lineno;
                    continue;
                }
                let Some(line) = Line::parse(text) else {
                    damage("the line is not a record line".into());
                    return Ok(());
                };
                if line.seq != self.total + 1 {
                    damage(format!(
                        "expected line number {}, found {}",
                        self.total + 1,
                        line.seq
                    ));
                    return Ok(());
                }
                self.pos += n as u64;
                self.lineno = lineno;
                self.total += 1;
                self.end = Some(End {
                    segment: si,
                    bytes: self.pos,
                });
                if self.total.is_multiple_of(MARK_EVERY) {
                    self.marks.push(Mark {
                        segment: si,
                        bytes: self.pos,
                        index: self.total,
                    });
                }
            }
            if newest {
                return Ok(());
            }
            let Some(id) = Ident::of(&path) else {
                return Err(DoorError::unreadable(
                    &path,
                    &io::Error::other("the segment vanished while it was read"),
                ));
            };
            self.done.push(id);
            self.cur_ino = None;
            self.seg += 1;
            self.pos = 0;
            self.lineno = 0;
        }
        Ok(())
    }

    fn mark_for(&self, offset: u64) -> Mark {
        let i = self.marks.partition_point(|m| m.index <= offset);
        self.marks[i.saturating_sub(1)]
    }
}

/// How long an index may go unused before it is dropped. A time bound on a
/// cache entry, not a limit on any record or text: a dropped index is built
/// again by the next page.
const INDEX_IDLE: std::time::Duration = std::time::Duration::from_secs(600);

type Slot = (std::sync::Arc<std::sync::Mutex<RecIdx>>, std::time::Instant);

fn indexes() -> &'static std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, Slot>> {
    static INDEXES: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, Slot>>,
    > = std::sync::OnceLock::new();
    INDEXES.get_or_init(Default::default)
}

/// Drop every index not used since `idle` before `now`, except one a page is
/// holding. Returns how many were dropped.
fn evict_idle(
    map: &mut std::collections::HashMap<std::path::PathBuf, Slot>,
    now: std::time::Instant,
    idle: std::time::Duration,
) -> usize {
    let before = map.len();
    map.retain(|_, (slot, used)| {
        std::sync::Arc::strong_count(slot) > 1 || now.saturating_duration_since(*used) < idle
    });
    before - map.len()
}

/// Forget the index of one state directory (a restart of the process, for a test).
#[doc(hidden)]
pub fn forget_record_index(state_dir: &Path) {
    indexes()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(state_dir);
}

/// Bring the index of the record in `state_dir` up to date and return what a
/// page needs: the scan result, and the mark to start reading line `offset` from.
fn snapshot(state_dir: &Path, offset: u64) -> Door<(ScanOut, Mark)> {
    check_state_dir(state_dir)?;
    let dir = state_dir.join("record");
    let now: Vec<std::path::PathBuf> = record::segments(&dir)
        .map_err(|e| DoorError::unreadable(&dir, &e))?
        .into_iter()
        .map(|(_, p)| p)
        .collect();
    let slot = {
        let mut map = indexes().lock().unwrap_or_else(|e| e.into_inner());
        let now = std::time::Instant::now();
        evict_idle(&mut map, now, INDEX_IDLE);
        let e = map
            .entry(state_dir.to_path_buf())
            .or_insert_with(|| (Default::default(), now));
        e.1 = now;
        e.0.clone()
    };
    let mut idx = slot.lock().unwrap_or_else(|e| e.into_inner());
    idx.advance(now)?;
    let out = ScanOut {
        scanned: Scanned {
            total: idx.total,
            damaged: idx.damaged.clone(),
        },
        segments: idx.segments.clone(),
        end: idx.end,
    };
    Ok((out, idx.mark_for(offset)))
}

/// The bytes of the intact prefix's non-blank lines, newest first, read from
/// the end of the record backwards in chunks. `each` gets one line's bytes
/// (no newline) and returns whether to go on. Memory is a chunk and one line.
fn lines_backwards(
    segments: &[std::path::PathBuf],
    end: End,
    mut each: impl FnMut(&[u8]) -> io::Result<bool>,
) -> io::Result<()> {
    const CHUNK: u64 = 64 * 1024;
    for si in (0..=end.segment).rev() {
        let mut f = File::open(&segments[si])?;
        let limit = if si == end.segment {
            end.bytes
        } else {
            f.metadata()?.len()
        };
        let mut pos = limit;
        if pos > 0 {
            let mut last = [0u8; 1];
            f.seek(SeekFrom::Start(pos - 1))?;
            f.read_exact(&mut last)?;
            if last[0] == b'\n' {
                pos -= 1;
            }
        }
        let mut buf: Vec<u8> = Vec::new();
        loop {
            while let Some(i) = buf.iter().rposition(|b| *b == b'\n') {
                if !each(&buf[i + 1..])? {
                    return Ok(());
                }
                buf.truncate(i);
            }
            if pos == 0 {
                break;
            }
            let start = pos.saturating_sub(CHUNK);
            let mut chunk = vec![0u8; (pos - start) as usize];
            f.seek(SeekFrom::Start(start))?;
            f.read_exact(&mut chunk)?;
            chunk.extend_from_slice(&buf);
            buf = chunk;
            pos = start;
        }
        if !each(&buf)? {
            return Ok(());
        }
    }
    Ok(())
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
pub(crate) fn env_names(v: &Value, out: &mut Vec<String>) {
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

    fn to_json(&self) -> Value {
        json!({"path": self.path, "content": self.content, "problem": self.problem})
    }
}

/// The redactor for a read: `base` (the gateway's own secrets) plus the value
/// of every variable that **any** registered instance's config names, since a
/// host can have been shown another's key. Also the names, sorted, of those
/// variables this process does not hold: their values cannot be found by value,
/// only by shape, so the answers list the names (never a value).
pub fn fleet_redactor(registry_dir: &Path, base: &Redactor) -> (Redactor, Vec<String>) {
    let mut red = base.clone();
    let mut names = Vec::new();
    for entry in registry::entries_in(registry_dir).unwrap_or_default() {
        if let Some(c) = ConfigView::read(&entry).content {
            env_names(&c, &mut names);
        }
    }
    names.sort();
    names.dedup();
    let mut unset = Vec::new();
    for n in names {
        if !red.add_env(&n) {
            unset.push(n);
        }
    }
    (red, unset)
}

impl DoorError {
    /// The error with its message through `red`: a path, an id or a query value
    /// in it can hold a key too.
    pub(crate) fn redacted(mut self, red: &Redactor) -> Self {
        self.message = red.redact(&self.message).into_owned();
        self
    }
}

// ─── finding an instance ─────────────────────────────────────────────────────

pub(crate) fn locate(registry_dir: &Path, id: &str) -> Door<(Row, Entry)> {
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

// ─── following an instance (H5) ──────────────────────────────────────────────

/// What a gateway needs to follow one native instance's stream.
#[derive(Debug, Clone)]
pub struct StreamTarget {
    pub state_dir: std::path::PathBuf,
    /// The host's state lock is held: a host is running on it.
    pub running: bool,
    /// Where the running host's own door is (`host:port`): only when its lock
    /// is held and its entry has an address.
    pub address: Option<String>,
    /// The host's own role tokens (values), the owner's first, as the
    /// gateway's process holds them: what the host's door takes.
    pub tokens: Vec<String>,
    /// The gateway's secrets and every config-named value of the registry.
    pub red: Redactor,
    pub env_unset: Vec<String>,
}

/// Find instance `id` for streaming. Errors are as the other doors', and as
/// redacted.
pub fn stream_target(registry_dir: &Path, id: &str, base: &Redactor) -> Door<StreamTarget> {
    let (red, env_unset) = fleet_redactor(registry_dir, base);
    let find = || -> Door<StreamTarget> {
        let (row, entry) = locate(registry_dir, id)?;
        check_state_dir(&entry.state_dir)?;
        let mut tokens = Vec::new();
        if let Some(c) = ConfigView::read(&entry).content
            && let Some(t) = c["acp"]["tokens"].as_object()
        {
            let mut named: Vec<(&String, &Value)> = t.iter().collect();
            named.sort_by_key(|(role, _)| role.as_str() != "owner");
            for (_, name) in named {
                if let Some(v) = name.as_str().and_then(|n| std::env::var(n).ok()) {
                    let v = v.trim().to_string();
                    if !v.is_empty() {
                        tokens.push(v);
                    }
                }
            }
        }
        Ok(StreamTarget {
            state_dir: entry.state_dir.clone(),
            running: row.word == Word::Running,
            address: (row.word == Word::Running)
                .then(|| entry.address.clone())
                .flatten(),
            tokens,
            red: red.clone(),
            env_unset: env_unset.clone(),
        })
    };
    find().map_err(|e| e.redacted(&red))
}

// ─── the queue (S1) ──────────────────────────────────────────────────────────

/// `GET /v1/queue` of native instance `id`: its waiting and answering items (and,
/// when asked, those over), computed from its own record, so a stopped instance
/// answers too. The shape and the paging are [`crate::queue::answer`]'s; every
/// text goes through the registry-wide redactor.
pub fn queue(registry_dir: &Path, id: &str, q: &crate::queue::Query, base: &Redactor) -> Door {
    let (red, env_unset) = fleet_redactor(registry_dir, base);
    let find = || -> Door {
        let (_row, entry) = locate(registry_dir, id)?;
        check_state_dir(&entry.state_dir)?;
        crate::queue::answer(&entry.state_dir, q, &red, &env_unset)
    };
    find().map_err(|e| e.redacted(&red))
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
fn summary_value(row: &Row, entry: &Entry, red: &Redactor, env_unset: &[String]) -> Door {
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
        "redaction": {"env_unset": env_unset},
        "fold_hash": st.hash(),
        "damaged": f.damaged.as_ref().map(Damaged::to_json),
        "problem": problem,
    });
    Ok(red.redact_value(&v))
}

/// `GET /v1/summary`: the fold of instance `id`, read from its own files.
pub fn summary(registry_dir: &Path, id: &str, base: &Redactor) -> Door {
    let (red, unset) = fleet_redactor(registry_dir, base);
    locate(registry_dir, id)
        .and_then(|(row, entry)| summary_value(&row, &entry, &red, &unset))
        .map_err(|e| e.redacted(&red))
}

// ─── the list ────────────────────────────────────────────────────────────────

/// Every instance in the registry folder, by id, as the list door shows it:
/// the registry's row (the entry's fields, `word`, `last`, `problem`), the
/// time of the last event, the command that starts it and its fold's summary
/// (null, with `error`, when it cannot be read). No row is dropped.
pub fn rows(registry_dir: &Path, base: &Redactor) -> Door<Vec<Value>> {
    let (red, unset) = fleet_redactor(registry_dir, base);
    let rows = registry::list_in(registry_dir)
        .map_err(|e| DoorError::new(500, "unreadable", e).redacted(&red))?;
    Ok(rows
        .iter()
        .map(|row| {
            let mut v = row.to_json();
            let summary = match &row.entry {
                Some(entry) => summary_value(row, entry, &red, &unset),
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
            red.redact_value(&v)
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

/// A record page ready to be written: everything that can be refused has been
/// (an unknown instance, a missing state directory, an unreadable record
/// directory), so [`write_json`](RecordPage::write_json) can start an answer.
#[derive(Debug)]
pub struct RecordPage {
    dir: std::path::PathBuf,
    offset: u64,
    q: RecordQuery,
    red: Redactor,
    env_unset: Vec<String>,
    /// For a newest-first page, the first scan: the intact prefix, found before
    /// the first byte is written.
    prefix: Option<ScanOut>,
}

/// `GET /v1/record`: the recorded events of instance `id`, one page. Oldest
/// first by default. The page is the caller's: `limit` lines from `offset` (or
/// after line number `after`); `total` is the number of intact lines; `next`
/// is the next offset or null on the last page; `cursor` is the `after` that
/// continues from this page. A cursor past the end is an empty last page.
///
/// A door asks for the page with [`record_open`] and writes it with
/// [`RecordPage::write_json`], which holds one line at a time however large the
/// page is.
///
/// Open the page `q` asks of instance `id`. A newest-first page first reads
/// the record once to find where it ends (nothing is held but its position);
/// an oldest-first page reads nothing yet.
pub fn record_open(
    registry_dir: &Path,
    id: &str,
    q: &RecordQuery,
    base: &Redactor,
) -> Door<RecordPage> {
    let (red, env_unset) = fleet_redactor(registry_dir, base);
    let open = || -> Door<RecordPage> {
        let (_, entry) = locate(registry_dir, id)?;
        let dir = entry.state_dir.clone();
        check_state_dir(&dir)?;
        let record_dir = dir.join("record");
        for (_, p) in
            record::segments(&record_dir).map_err(|e| DoorError::unreadable(&record_dir, &e))?
        {
            File::open(&p).map_err(|e| DoorError::unreadable(&p, &e))?;
        }
        let offset = q.after.unwrap_or(q.offset);
        let prefix = if q.desc {
            Some(snapshot(&dir, 0)?.0)
        } else {
            None
        };
        Ok(RecordPage {
            dir,
            offset,
            q: q.clone(),
            red: red.clone(),
            env_unset: env_unset.clone(),
            prefix,
        })
    };
    open().map_err(|e| e.redacted(&red))
}

impl RecordPage {
    /// Write the page as one JSON object, a line at a time: each line is
    /// redacted and written before the next is read, so the memory held does
    /// not grow with the page (one line, however long, plus a chunk). `lines`
    /// comes first; the fields that need the whole read (`total`, `next`,
    /// `damaged`) follow it.
    pub fn write_json(self, out: &mut dyn Write) -> io::Result<()> {
        out.write_all(b"{\"lines\":[")?;
        let mut written = 0u64;
        /// One line, redacted, after a comma unless it is the first.
        fn put(out: &mut dyn Write, l: &Line, red: &Redactor, written: &mut u64) -> io::Result<()> {
            if *written > 0 {
                out.write_all(b",")?;
            }
            serde_json::to_writer(&mut *out, &red.redact_value(&l.to_value()))?;
            *written += 1;
            Ok(())
        }
        let (total, damaged) = match &self.prefix {
            None => {
                // oldest first: seek to the mark at or before `offset` and read
                // forward, writing the window as it is read; the total is the
                // index's, which read only what was appended since the last page
                let (lo, hi) = (self.offset, self.offset.saturating_add(self.q.limit));
                let (s, mark) = snapshot(&self.dir, lo).map_err(|e| io::Error::other(e.message))?;
                let total = s.scanned.total;
                let mut at = mark.index;
                let mut si = mark.segment;
                let mut from = mark.bytes;
                let mut buf = Vec::new();
                'read: while lo < total && at < hi.min(total) && si < s.segments.len() {
                    let mut f = File::open(&s.segments[si])?;
                    f.seek(SeekFrom::Start(from))?;
                    let mut r = BufReader::new(f);
                    loop {
                        buf.clear();
                        if r.read_until(b'\n', &mut buf)? == 0 {
                            break;
                        }
                        count_line();
                        let Ok(text) = std::str::from_utf8(&buf) else {
                            break 'read;
                        };
                        let text = text.trim_end_matches(['\n', '\r']);
                        if text.trim().is_empty() {
                            continue;
                        }
                        let Some(l) = Line::parse(text) else {
                            break 'read;
                        };
                        if l.seq != at + 1 {
                            return Err(io::Error::other(format!(
                                "the record changed under the page: expected line number {}, found {}",
                                at + 1,
                                l.seq
                            )));
                        }
                        if at >= lo {
                            put(&mut *out, &l, &self.red, &mut written)?;
                        }
                        at += 1;
                        if at >= hi.min(total) {
                            break 'read;
                        }
                    }
                    si += 1;
                    from = 0;
                }
                (total, s.scanned.damaged)
            }
            Some(first) => {
                // newest first: read backwards from where the first scan ended,
                // skipping `offset` lines, writing `limit`
                let mut skip = self.offset;
                if let Some(end) = first.end {
                    lines_backwards(&first.segments, end, |raw| {
                        let Ok(text) = std::str::from_utf8(raw) else {
                            return Ok(true);
                        };
                        let text = text.trim_end_matches('\r');
                        if text.trim().is_empty() {
                            return Ok(true);
                        }
                        if skip > 0 {
                            skip -= 1;
                            return Ok(true);
                        }
                        let Some(l) = Line::parse(text) else {
                            return Ok(true);
                        };
                        put(&mut *out, &l, &self.red, &mut written)?;
                        Ok(written < self.q.limit)
                    })?;
                }
                (first.scanned.total, first.scanned.damaged.clone())
            }
        };
        let end = self.offset.saturating_add(written);
        let trailer = json!({
            "offset": self.offset,
            "limit": self.q.limit,
            "total": total,
            "next": (end < total).then_some(end),
            "cursor": (!self.q.desc).then_some(end),
            "order": if self.q.desc { "desc" } else { "asc" },
            "after": self.q.after,
            "damaged": damaged.as_ref().map(Damaged::to_json),
            "redaction": {"env_unset": self.env_unset},
        });
        let text = serde_json::to_string(&self.red.redact_value(&trailer))?;
        out.write_all(b"],")?;
        out.write_all(&text.as_bytes()[1..])?;
        out.flush()
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn an_index_idle_too_long_is_dropped_and_one_in_use_or_fresh_is_kept() {
        let t0 = Instant::now();
        let mut m = std::collections::HashMap::new();
        let fresh: Slot = (Default::default(), t0 + Duration::from_secs(95));
        let idle: Slot = (Default::default(), t0);
        let held: Slot = (Default::default(), t0);
        let _page = held.0.clone(); // a page is reading it
        m.insert("fresh".into(), fresh);
        m.insert("idle".into(), idle);
        m.insert("held".into(), held);
        let n = evict_idle(
            &mut m,
            t0 + Duration::from_secs(100),
            Duration::from_secs(10),
        );
        assert_eq!(n, 1);
        assert!(m.contains_key(std::path::Path::new("fresh")));
        assert!(m.contains_key(std::path::Path::new("held")));
        assert!(!m.contains_key(std::path::Path::new("idle")));
    }
}
