//! The event stream with replay (design note H5): an instance's record,
//! followed, plus short-lived partial frames of the turn that is running.
//!
//! Two kinds of frame, kept apart on purpose:
//!
//! - **record frames** are the record's lines, numbered by their record
//!   number. They are read from the record on disk by one reader per client,
//!   from the client's cursor to the end and then as they are appended: replay
//!   and live are the same read, so nothing is missed and nothing is repeated
//!   at the seam, and a stopped instance replays the same way. A client that
//!   reads slowly just reads later: nothing is dropped, nothing queues.
//! - **partial frames** (`delta` events) carry the partial text and tool
//!   progress of the turn that is running. They exist only in memory, only
//!   for the clients connected when they happen: never written to the record,
//!   never replayed, never numbered.
//!
//! Every frame is passed through the shared redactor before it is written.
//! Text that arrives in pieces is redacted across the pieces: a secret split
//! between two partial frames is still removed ([`PartialRedactor`]).
//!
//! Nothing here blocks a thread for a client: [`Events::next`] is an async
//! pull, so the transport's own flow control (a client that does not read)
//! stops the reads, and a dropped client drops its file and subscription.
//! Door shapes: `docs/rung-host-api.md` (H5).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::sync::{Notify, broadcast, mpsc};

use crate::read::{Door, DoorError};
use crate::record::Line;
use crate::redact::Redactor;

/// The stream contract's version, stated in the first frame.
pub const CONTRACT: u32 = 1;

/// How often the record is looked at again when nothing woke the stream: the
/// bound on how late a line written by another process (a gateway reading a
/// host's state directory) reaches a client.
pub const POLL: Duration = Duration::from_millis(100);

/// How often an idle stream writes a comment line, so a proxy keeps the path
/// open.
pub const KEEPALIVE: Duration = Duration::from_secs(15);

/// Partial frames a subscriber may be behind before it is told it fell behind.
const BUS_CAPACITY: usize = 4096;

// ─── partial frames ──────────────────────────────────────────────────────────

/// One partial frame: what the running turn is doing right now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Partial {
    /// The turn it belongs to (its `turn.started` line's `turn`).
    pub turn: u64,
    /// The model call within the turn, from 0.
    pub call: u32,
    #[serde(flatten)]
    pub what: What,
}

/// What a partial frame says. `kind` on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum What {
    /// More of the answer text.
    Text { text: String },
    /// The model began a tool call.
    ToolCall { id: String, name: String },
    /// More of a tool call's arguments (JSON text, in pieces).
    ToolInput { id: String, delta: String },
    /// The tool is being run with these arguments.
    ToolRun { name: String, input: Value },
    /// The tool finished.
    ToolDone {
        name: String,
        ok: bool,
        output: String,
    },
    /// A text block begins (a tool call's arguments begin with its
    /// `tool_call`). The text of a block is sent only to a client that saw its
    /// start (see [`PartialRedactor`]).
    BlockStart { id: Option<String> },
    /// A text block (`id` null) or a tool call's arguments (`id`) is complete:
    /// anything held back for redaction is released just before this frame.
    BlockEnd { id: Option<String> },
    /// The turn's engine returned: everything of the turn is released.
    TurnEnd,
    /// This subscriber missed `skipped` partial frames (it read too slowly).
    /// Never sent by an engine; the stream makes it.
    Behind { skipped: u64 },
}

/// The in-memory side of the stream: where a running turn publishes partial
/// frames, and what wakes a stream when the record grows. One per host.
#[derive(Debug)]
pub struct LiveBus {
    tx: broadcast::Sender<Arc<Partial>>,
    wake: Notify,
}

impl LiveBus {
    pub fn new() -> Arc<Self> {
        let (tx, _) = broadcast::channel(BUS_CAPACITY);
        Arc::new(Self {
            tx,
            wake: Notify::new(),
        })
    }

    /// Publish a partial frame to whoever is connected now. Never blocks and
    /// never fails: with no one connected the frame is simply gone.
    pub fn publish(&self, p: Partial) {
        let _ = self.tx.send(Arc::new(p));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Partial>> {
        self.tx.subscribe()
    }

    /// How many streams are connected.
    pub fn subscribers(&self) -> usize {
        self.tx.receiver_count()
    }

    /// The record grew: wake the streams waiting for it.
    pub fn wake(&self) {
        self.wake.notify_waiters();
    }

    /// The handle one turn's engine publishes through.
    pub fn turn(self: &Arc<Self>, turn: u64) -> LiveTurn {
        LiveTurn {
            bus: Some(self.clone()),
            turn,
        }
    }
}

/// What an engine publishes partial frames through, for one turn. A turn with
/// no bus ([`LiveTurn::none`]) publishes nothing.
#[derive(Debug, Clone)]
pub struct LiveTurn {
    bus: Option<Arc<LiveBus>>,
    turn: u64,
}

impl LiveTurn {
    pub fn none() -> Self {
        Self { bus: None, turn: 0 }
    }

    /// Is anyone connected? An engine may skip building a frame if not.
    pub fn watched(&self) -> bool {
        self.bus.as_ref().is_some_and(|b| b.subscribers() > 0)
    }

    fn put(&self, call: u32, what: What) {
        if let Some(b) = &self.bus {
            b.publish(Partial {
                turn: self.turn,
                call,
                what,
            });
        }
    }

    pub fn text(&self, call: u32, text: &str) {
        self.put(call, What::Text { text: text.into() });
    }

    pub fn tool_call(&self, call: u32, id: &str, name: &str) {
        self.put(
            call,
            What::ToolCall {
                id: id.into(),
                name: name.into(),
            },
        );
    }

    pub fn tool_input(&self, call: u32, id: &str, delta: &str) {
        self.put(
            call,
            What::ToolInput {
                id: id.into(),
                delta: delta.into(),
            },
        );
    }

    pub fn tool_run(&self, call: u32, name: &str, input: &Value) {
        self.put(
            call,
            What::ToolRun {
                name: name.into(),
                input: input.clone(),
            },
        );
    }

    pub fn tool_done(&self, call: u32, name: &str, ok: bool, output: &str) {
        self.put(
            call,
            What::ToolDone {
                name: name.into(),
                ok,
                output: output.into(),
            },
        );
    }

    /// The answer text of `call` begins: publish before its first `text`.
    pub fn block_start(&self, call: u32) {
        self.put(call, What::BlockStart { id: None });
    }

    pub fn block_end(&self, call: u32, id: Option<&str>) {
        self.put(
            call,
            What::BlockEnd {
                id: id.map(String::from),
            },
        );
    }

    pub fn end(&self) {
        self.put(0, What::TurnEnd);
    }
}

// ─── frames ──────────────────────────────────────────────────────────────────

/// One thing a stream says to its client.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// First frame: where the stream starts and what it carries.
    Open {
        after: u64,
        live: bool,
        partials: bool,
        /// The variables the config names that this process does not hold:
        /// a value held there cannot be found by value (see `read`).
        env_unset: Vec<String>,
    },
    /// A record line (already redacted).
    Record(Value),
    /// A partial frame (already redacted).
    Delta(Value),
    /// An idle stream still open.
    Ping,
    /// The last frame: why the stream ends.
    End {
        reason: String,
        last_seq: u64,
        problem: Option<String>,
    },
}

impl Frame {
    /// The frame as server-sent-event text: record frames carry their record
    /// number as the event id; no other frame has an id, so a reconnect
    /// resumes from the last record line seen.
    pub fn sse(&self) -> String {
        match self {
            Frame::Open {
                after,
                live,
                partials,
                env_unset,
            } => format!(
                "event: open\ndata: {}\n\n",
                json!({"contract": CONTRACT, "after": after, "live": live, "partials": partials,
                       "redaction": {"env_unset": env_unset}})
            ),
            Frame::Record(v) => {
                let id = v["seq"].as_u64().unwrap_or(0);
                format!("event: record\nid: {id}\ndata: {v}\n\n")
            }
            Frame::Delta(v) => format!("event: delta\ndata: {v}\n\n"),
            Frame::Ping => ": keepalive\n\n".to_string(),
            Frame::End {
                reason,
                last_seq,
                problem,
            } => format!(
                "event: end\ndata: {}\n\n",
                json!({"reason": reason, "last_seq": last_seq, "problem": problem})
            ),
        }
    }
}

// ─── redacting text that arrives in pieces ───────────────────────────────────

/// What a continuation would look like to the redactor: a long word with no
/// shape of its own. If appending it to some text changes how the redactor
/// sees that text, the text ends inside a pattern that is still open (an
/// `Authorization:` header, `NAME=` waiting for its value, an open private
/// key block), and nothing from where the pattern began may go out yet.
const PROBE: &str = "zq0123456789abcdefghijklmnopqrstuvwxyzqz";

/// Redacts partial frames across their pieces: text and tool-call arguments
/// arrive a few characters at a time, and a secret may be split between two
/// pieces, where no single piece holds it. So the redactor holds back the part
/// of a block's text that could still be the start of something it removes,
/// and releases the rest as soon as it is settled:
///
/// - the last word (a key shape is a word: it is complete only when something
///   follows it),
/// - the last `longest secret - 1` bytes (an exact value is found only whole),
/// - and the whole line, or block, a pattern left open (see the probe below).
///
/// What is held is released, redacted, when the block ends or the turn ends.
/// Nothing is dropped and no frame is cut: the text that goes out, joined, is
/// what the redactor makes of the whole text at once.
///
/// **It sends the text of a block only if it saw the block begin** (a
/// `block_start`, or a `tool_call` for a call's arguments), and holds that
/// state per client. A client that joins in the middle of a block, or that fell
/// behind (`behind`) while one was open, has not seen the text a secret may have
/// begun in, and a redactor that did not see it cannot know the rest of the
/// secret: so the rest of that block is not sent, the held text is discarded
/// (not released: a held piece may be the front of a secret), and the stream
/// resumes at the next block, which it sees from its start. The block's whole
/// text is in the record.
#[derive(Debug)]
pub struct PartialRedactor {
    red: Redactor,
    /// Held text, by block: (turn, call, tool-call id or none for the answer).
    held: BTreeMap<BlockKey, String>,
    /// The blocks whose start this redactor has seen and whose end it has not.
    /// Only their text is sent: see the type's docs.
    started: BTreeSet<BlockKey>,
}

/// (turn, call, tool-call id, or none for the answer text).
type BlockKey = (u64, u32, Option<String>);

impl PartialRedactor {
    pub fn new(red: Redactor) -> Self {
        Self {
            red,
            held: BTreeMap::new(),
            started: BTreeSet::new(),
        }
    }

    /// How much of `buf` is settled: the byte index up to which it can go out
    /// now.
    fn settled(&self, buf: &str) -> usize {
        let mut limit = buf
            .len()
            .saturating_sub(self.red.longest_secret().saturating_sub(1));
        while !buf.is_char_boundary(limit) {
            limit -= 1;
        }
        let Some((i, c)) = buf[..limit]
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
        else {
            return 0;
        };
        let mut p = i + c.len_utf8();
        // a private key block is removed only whole: while one is open, its
        // first line and all after it stay
        if let Some(b) = buf[..p].rfind("-----BEGIN ")
            && !buf[b..p].contains("-----END ")
        {
            p = buf[..b].rfind('\n').map(|n| n + 1).unwrap_or(0);
        }
        while p > 0 && !self.closed(&buf[..p]) {
            let s = buf[..p].strip_suffix('\n').unwrap_or(&buf[..p]);
            p = s.rfind('\n').map(|n| n + 1).unwrap_or(0);
        }
        p
    }

    /// Does `text` end with no pattern left open?
    fn closed(&self, text: &str) -> bool {
        self.red.redact(&format!("{text}{PROBE}")).ends_with(PROBE)
    }

    fn text_frame(&self, p: &Partial, id: &Option<String>, text: String) -> Value {
        match id {
            None => json!({"turn": p.turn, "call": p.call, "kind": "text", "text": text}),
            Some(id) => json!({"turn": p.turn, "call": p.call, "kind": "tool_input",
                               "id": self.red.redact(id), "delta": text}),
        }
    }

    /// Release what is held for the blocks `which` selects.
    fn release(&mut self, which: impl Fn(&BlockKey) -> bool, out: &mut Vec<Value>) {
        let keys: Vec<_> = self.held.keys().filter(|k| which(k)).cloned().collect();
        for k in keys {
            let buf = self.held.remove(&k).unwrap_or_default();
            if buf.is_empty() {
                continue;
            }
            let p = Partial {
                turn: k.0,
                call: k.1,
                what: What::BlockEnd { id: None },
            };
            out.push(self.text_frame(&p, &k.2, self.red.redact(&buf).into_owned()));
        }
    }

    /// One partial frame in; the frames that may go out now, redacted.
    pub fn push(&mut self, p: Partial) -> Vec<Value> {
        let mut out = Vec::new();
        match &p.what {
            What::Text { text } => {
                if self.started.contains(&(p.turn, p.call, None)) {
                    self.piece(&p, None, text, &mut out);
                }
            }
            What::ToolInput { id, delta } => {
                if self.started.contains(&(p.turn, p.call, Some(id.clone()))) {
                    self.piece(&p, Some(id.clone()), delta, &mut out);
                }
            }
            What::BlockStart { id } => {
                self.started.insert((p.turn, p.call, id.clone()));
                out.push(self.whole(&p));
            }
            What::ToolCall { id, .. } => {
                self.started.insert((p.turn, p.call, Some(id.clone())));
                out.push(self.whole(&p));
            }
            What::BlockEnd { id } => {
                // the end of a block whose start was not seen is not sent
                let key = (p.turn, p.call, id.clone());
                if self.started.remove(&key) {
                    self.release(|k| *k == key, &mut out);
                    out.push(self.whole(&p));
                }
            }
            What::TurnEnd => {
                let t = p.turn;
                self.release(|k| k.0 == t, &mut out);
                self.started.retain(|k| k.0 != t);
                out.push(self.whole(&p));
            }
            What::Behind { .. } => {
                // frames were missed: no block open is whole any more
                self.held.clear();
                self.started.clear();
                out.push(self.whole(&p));
            }
            What::ToolRun { .. } | What::ToolDone { .. } => out.push(self.whole(&p)),
        }
        out
    }

    /// A frame whose text is complete in itself: every string in it redacted.
    fn whole(&self, p: &Partial) -> Value {
        let v = serde_json::to_value(p).unwrap_or(Value::Null);
        self.red.redact_value(&v)
    }

    fn piece(&mut self, p: &Partial, id: Option<String>, text: &str, out: &mut Vec<Value>) {
        let key = (p.turn, p.call, id.clone());
        let mut buf = self.held.remove(&key).unwrap_or_default();
        buf.push_str(text);
        let upto = self.settled(&buf);
        if upto > 0 {
            let rest = buf.split_off(upto);
            let said = self.red.redact(&buf).into_owned();
            out.push(self.text_frame(p, &id, said));
            buf = rest;
        }
        if !buf.is_empty() {
            self.held.insert(key, buf);
        }
    }
}

// ─── the record, tailed ──────────────────────────────────────────────────────

/// What one read of the tail found.
enum Item {
    Line(Line),
    /// Nothing more written yet.
    Idle,
    /// The record stops being intact here.
    Damaged(String),
}

struct Cur {
    n: u32,
    file: tokio::fs::File,
    buf: Vec<u8>,
    /// Where the unread part of `buf` begins.
    off: usize,
    /// How far into `buf` there is known to be no newline.
    scanned: usize,
    /// The read buffer, kept between reads.
    chunk: Vec<u8>,
}

impl Cur {
    /// The `n` bytes just read into `chunk` join the unread part of `buf`; what
    /// was consumed before them is let go first, so a client holds what it has
    /// not yet handled and nothing it has.
    fn append(&mut self, n: usize) {
        if self.off > 0 {
            self.buf.drain(..self.off);
            self.scanned = self.scanned.saturating_sub(self.off);
            self.off = 0;
        }
        self.buf.extend_from_slice(&self.chunk[..n]);
    }
}

/// Follows the record's segments from a cursor, reading only what is there.
struct Tail {
    dir: PathBuf,
    after: u64,
    cur: Option<Cur>,
    /// The last line number read (skipped lines included).
    last_seq: u64,
    last_kind: String,
    /// Is `last_seq` known? False until the first line of a segment that does
    /// not begin the record has been read.
    anchored: bool,
}

fn segment_file(dir: &Path, n: u32) -> PathBuf {
    dir.join(format!("seg-{n:08}.ndjson"))
}

fn io_problem(what: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", what.display())
}

/// The record's segment numbers, ascending. No directory is none.
async fn segment_numbers(dir: &Path) -> std::io::Result<Vec<u32>> {
    let mut out = Vec::new();
    let mut rd = match tokio::fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e),
    };
    while let Some(e) = rd.next_entry().await? {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if let Some(n) = name
            .strip_prefix("seg-")
            .and_then(|s| s.strip_suffix(".ndjson"))
            .and_then(|s| s.parse::<u32>().ok())
        {
            out.push(n);
        }
    }
    out.sort_unstable();
    Ok(out)
}

/// The number of the first line of a segment, if it has a whole one.
async fn first_seq(path: &Path) -> Option<u64> {
    let mut f = tokio::fs::File::open(path).await.ok()?;
    let mut line = Vec::new();
    let mut chunk = vec![0u8; 8192];
    loop {
        let n = f.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        if let Some(i) = chunk[..n].iter().position(|b| *b == b'\n') {
            line.extend_from_slice(&chunk[..i]);
            break;
        }
        line.extend_from_slice(&chunk[..n]);
    }
    Line::parse(std::str::from_utf8(&line).ok()?).map(|l| l.seq)
}

impl Tail {
    fn new(state_dir: &Path, after: u64) -> Self {
        Self {
            dir: state_dir.join("record"),
            after,
            cur: None,
            last_seq: 0,
            last_kind: String::new(),
            anchored: false,
        }
    }

    /// Open the segment the cursor falls in: the last one whose first line is
    /// at or before the line after the cursor.
    async fn open_first(&mut self) -> Result<bool, String> {
        let nums = segment_numbers(&self.dir)
            .await
            .map_err(|e| io_problem(&self.dir, &e))?;
        let Some(&first) = nums.first() else {
            return Ok(false);
        };
        let mut pick = first;
        if self.after > 0 {
            for &n in &nums[1..] {
                match first_seq(&segment_file(&self.dir, n)).await {
                    Some(s) if s <= self.after + 1 => pick = n,
                    _ => break,
                }
            }
        }
        self.anchored = pick == first;
        self.last_seq = 0;
        self.open_segment(pick).await?;
        Ok(true)
    }

    async fn open_segment(&mut self, n: u32) -> Result<(), String> {
        let path = segment_file(&self.dir, n);
        let file = tokio::fs::File::open(&path)
            .await
            .map_err(|e| io_problem(&path, &e))?;
        self.cur = Some(Cur {
            n,
            file,
            buf: Vec::new(),
            off: 0,
            scanned: 0,
            chunk: vec![0u8; 64 * 1024],
        });
        Ok(())
    }

    /// The next line after the cursor, or why there is none yet.
    async fn read(&mut self) -> Result<Item, String> {
        loop {
            if self.cur.is_none() && !self.open_first().await? {
                return Ok(Item::Idle);
            }
            let cur = self.cur.as_mut().expect("opened");
            // a whole line already in hand?
            if let Some(i) = cur.buf[cur.scanned.max(cur.off)..]
                .iter()
                .position(|b| *b == b'\n')
            {
                let end = cur.scanned.max(cur.off) + i;
                let raw = cur.buf[cur.off..end].to_vec();
                cur.off = end + 1;
                cur.scanned = cur.off;
                if cur.off == cur.buf.len() {
                    cur.buf.clear();
                    cur.off = 0;
                    cur.scanned = 0;
                }
                let Ok(text) = std::str::from_utf8(&raw) else {
                    return Ok(Item::Damaged("the line is not UTF-8".into()));
                };
                let text = text.trim_end_matches('\r');
                if text.trim().is_empty() {
                    continue;
                }
                let Some(line) = Line::parse(text) else {
                    return Ok(Item::Damaged("the line is not a record line".into()));
                };
                if !self.anchored {
                    self.last_seq = line.seq.saturating_sub(1);
                    self.anchored = true;
                }
                if line.seq != self.last_seq + 1 {
                    return Ok(Item::Damaged(format!(
                        "expected line number {}, found {}",
                        self.last_seq + 1,
                        line.seq
                    )));
                }
                self.last_seq = line.seq;
                self.last_kind.clone_from(&line.kind);
                if line.seq <= self.after {
                    continue; // before the cursor
                }
                return Ok(Item::Line(line));
            }
            cur.scanned = cur.buf.len();
            // more bytes in this segment?
            let n = cur
                .file
                .read(&mut cur.chunk)
                .await
                .map_err(|e| io_problem(&segment_file(&self.dir, cur.n), &e))?;
            if n > 0 {
                cur.append(n);
                continue;
            }
            // the end of this segment as it is now: has the host moved on?
            let next = segment_file(&self.dir, cur.n + 1);
            if !tokio::fs::try_exists(&next).await.unwrap_or(false) {
                return Ok(Item::Idle);
            }
            // it has, so this segment is final: take what it still holds
            let n = cur
                .file
                .read(&mut cur.chunk)
                .await
                .map_err(|e| io_problem(&segment_file(&self.dir, cur.n), &e))?;
            if n > 0 {
                cur.append(n);
                continue;
            }
            if cur.off < cur.buf.len() {
                return Ok(Item::Damaged(
                    "a segment ends in the middle of a line".into(),
                ));
            }
            let to = cur.n + 1;
            self.open_segment(to).await?;
        }
    }
}

// ─── the stream ──────────────────────────────────────────────────────────────

/// Which partial frames a stream carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Partials {
    /// Record and partial frames (the default).
    On,
    /// Record frames only.
    Off,
    /// Partial frames only: no record is read.
    Only,
}

/// How a stream is opened.
pub struct Options {
    /// Start after this record number (0: from the start).
    pub after: u64,
    pub partials: Partials,
    pub red: Redactor,
    /// Named in the first frame: see [`Frame::Open`].
    pub env_unset: Vec<String>,
    /// The host's own bus (a host serving its own door).
    pub bus: Option<Arc<LiveBus>>,
    /// Partial frames from elsewhere (a gateway relaying a host's).
    pub upstream: Option<mpsc::Receiver<Partial>>,
    /// See [`POLL`].
    pub poll: Duration,
    /// See [`KEEPALIVE`].
    pub keepalive: Duration,
}

impl Options {
    pub fn new(red: Redactor) -> Self {
        Self {
            after: 0,
            partials: Partials::On,
            red,
            env_unset: Vec::new(),
            bus: None,
            upstream: None,
            poll: POLL,
            keepalive: KEEPALIVE,
        }
    }
}

enum Source {
    Bus(broadcast::Receiver<Arc<Partial>>),
    Chan(mpsc::Receiver<Partial>),
}

impl Source {
    /// The next partial frame; a subscriber that lagged is told how far.
    /// `None`: the source is gone.
    async fn recv(&mut self) -> Option<Partial> {
        match self {
            Source::Bus(rx) => match rx.recv().await {
                Ok(p) => Some((*p).clone()),
                Err(broadcast::error::RecvError::Lagged(n)) => Some(Partial {
                    turn: 0,
                    call: 0,
                    what: What::Behind { skipped: n },
                }),
                Err(broadcast::error::RecvError::Closed) => None,
            },
            Source::Chan(rx) => rx.recv().await,
        }
    }
}

/// An open stream: ask it for its next frame.
pub struct Events {
    dir: PathBuf,
    tail: Option<Tail>,
    bus: Option<Arc<LiveBus>>,
    source: Option<Source>,
    pr: PartialRedactor,
    red: Redactor,
    queue: VecDeque<Frame>,
    open: Option<Frame>,
    done: bool,
    poll: Duration,
    keepalive: Duration,
}

impl Events {
    /// Open a stream over the record in `state_dir`. A missing state
    /// directory, or a cursor past the end of the record, is refused here,
    /// before any frame.
    pub async fn open(state_dir: &Path, opts: Options) -> Door<Events> {
        let red = opts.red.clone();
        let open = || async {
            crate::read::check_state_dir(state_dir)?;
            let reading = opts.partials != Partials::Only;
            if reading && opts.after > 0 {
                let dir = state_dir.join("record");
                let last =
                    tokio::task::spawn_blocking(move || crate::record::Record::last_line(dir))
                        .await
                        .map_err(|e| DoorError::new(500, "unreadable", e.to_string()))?
                        .map_err(|e| DoorError::new(500, "unreadable", format!("record: {e}")))?;
                let end = last.map(|l| l.seq).unwrap_or(0);
                if opts.after > end {
                    return Err(DoorError::new(
                        409,
                        "cursor_ahead",
                        format!(
                            "the cursor {} is past the record's last line {end}",
                            opts.after
                        ),
                    ));
                }
            }
            Ok(())
        };
        open().await.map_err(|e: DoorError| DoorError {
            message: red.redact(&e.message).into_owned(),
            ..e
        })?;
        // subscribe before anything is read: a frame published during the
        // replay is delivered, not missed
        let source = match opts.partials {
            Partials::Off => None,
            _ => match (&opts.bus, opts.upstream) {
                (Some(b), _) => Some(Source::Bus(b.subscribe())),
                (None, Some(rx)) => Some(Source::Chan(rx)),
                (None, None) => None,
            },
        };
        let live = crate::registry::state_held(state_dir).unwrap_or(false);
        Ok(Events {
            dir: state_dir.to_path_buf(),
            tail: (opts.partials != Partials::Only).then(|| Tail::new(state_dir, opts.after)),
            bus: opts.bus,
            pr: PartialRedactor::new(opts.red.clone()),
            red: opts.red,
            queue: VecDeque::new(),
            open: Some(Frame::Open {
                after: opts.after,
                live: live || opts.partials == Partials::Only,
                partials: source.is_some(),
                env_unset: opts.env_unset,
            }),
            source,
            done: false,
            poll: opts.poll,
            keepalive: opts.keepalive,
        })
    }

    fn end(&mut self, reason: &str, problem: Option<String>) {
        let last_seq = self.tail.as_ref().map(|t| t.last_seq).unwrap_or(0);
        self.queue.push_back(Frame::End {
            reason: reason.into(),
            last_seq,
            problem: problem.map(|p| self.red.redact(&p).into_owned()),
        });
    }

    fn take(&mut self, p: Partial) {
        for v in self.pr.push(p) {
            self.queue.push_back(Frame::Delta(v));
        }
    }

    /// The next frame, waiting for it; `None` after the `end` frame.
    pub async fn next(&mut self) -> Option<Frame> {
        let mut keep = std::pin::pin!(tokio::time::sleep(self.keepalive));
        loop {
            if let Some(f) = self.queue.pop_front() {
                if matches!(f, Frame::End { .. }) {
                    // the stream is over: let go of the subscription now
                    self.done = true;
                    self.source = None;
                    self.tail = None;
                }
                return Some(f);
            }
            if self.done {
                return None;
            }
            if let Some(f) = self.open.take() {
                return Some(f);
            }
            // arm the wake before looking, so a line written between the look
            // and the wait is not missed
            let wake = self.bus.clone();
            let mut notified = std::pin::pin!(wake.as_ref().map(|b| b.wake.notified()));
            if let Some(n) = notified.as_mut().as_pin_mut() {
                n.enable();
            }
            if let Some(frame) = self.look().await {
                return Some(frame);
            }
            if !self.queue.is_empty() || self.done {
                continue;
            }
            tokio::select! {
                _ = async {
                    match notified.as_mut().as_pin_mut() {
                        Some(n) => n.await,
                        None => std::future::pending().await,
                    }
                } => {}
                p = recv_or_pending(&mut self.source) => self.partial(p),
                _ = async {
                    // a stream with no record to read has nothing to poll
                    if self.tail.is_some() {
                        tokio::time::sleep(self.poll).await
                    } else {
                        std::future::pending().await
                    }
                } => {}
                _ = &mut keep => return Some(Frame::Ping),
            }
        }
    }

    fn partial(&mut self, p: Option<Partial>) {
        match p {
            Some(p) => self.take(p),
            // the source is gone: the record goes on without partial frames
            None => self.source = None,
        }
    }

    /// Look at the record once: a line to send, or the end, or nothing yet.
    async fn look(&mut self) -> Option<Frame> {
        let tail = self.tail.as_mut()?;
        match tail.read().await {
            Ok(Item::Line(l)) => Some(Frame::Record(self.red.redact_value(&l.to_value()))),
            Ok(Item::Damaged(problem)) => {
                self.end("damaged", Some(problem));
                None
            }
            Err(problem) => {
                self.end("error", Some(problem));
                None
            }
            Ok(Item::Idle) => {
                // a stopped host writes nothing more; but a line written
                // before it stopped may have come after our look, so look
                // again once the lock is seen free
                match crate::registry::state_held(&self.dir) {
                    Ok(true) => None,
                    Ok(false) => {
                        let tail = self.tail.as_mut()?;
                        match tail.read().await {
                            Ok(Item::Line(l)) => {
                                Some(Frame::Record(self.red.redact_value(&l.to_value())))
                            }
                            Ok(Item::Idle) => {
                                let reason = if tail.last_kind == "halted" {
                                    "stopped"
                                } else {
                                    "down"
                                };
                                self.end(reason, None);
                                None
                            }
                            Ok(Item::Damaged(p)) => {
                                self.end("damaged", Some(p));
                                None
                            }
                            Err(p) => {
                                self.end("error", Some(p));
                                None
                            }
                        }
                    }
                    Err(problem) => {
                        self.end("error", Some(problem));
                        None
                    }
                }
            }
        }
    }
}

async fn recv_or_pending(source: &mut Option<Source>) -> Option<Partial> {
    match source {
        Some(s) => s.recv().await,
        None => std::future::pending().await,
    }
}

// ─── the door's parameters and body ──────────────────────────────────────────

/// The query of an events door: `after` (a record number) and `partials`
/// (`on`, `off` or `only`). The `Last-Event-ID` header, if sent, is the cursor
/// instead of `after`: a client that reconnects names the last line it saw,
/// while the address it was given still carries its first `after`.
#[derive(Debug, Clone, PartialEq)]
pub struct EventsQuery {
    pub after: u64,
    pub partials: Partials,
}

impl EventsQuery {
    /// Unknown or repeated names, and values that are not as below, are 400.
    pub fn parse(pairs: &[(String, String)], last_event_id: Option<&str>) -> Door<Self> {
        let bad = |m: String| DoorError::new(400, "bad_request", m);
        let mut q = EventsQuery {
            after: 0,
            partials: Partials::On,
        };
        let mut seen: Vec<&str> = Vec::new();
        for (k, v) in pairs {
            if seen.contains(&k.as_str()) {
                return Err(bad(format!("{k} is given twice")));
            }
            seen.push(k);
            match k.as_str() {
                "after" => q.after = whole(k, v).map_err(bad)?,
                "partials" => {
                    q.partials = match v.as_str() {
                        "on" => Partials::On,
                        "off" => Partials::Off,
                        "only" => Partials::Only,
                        _ => return Err(bad("partials is on, off or only".into())),
                    }
                }
                other => {
                    return Err(bad(format!(
                        "no such parameter {other:?} (after, partials)"
                    )));
                }
            }
        }
        if let Some(id) = last_event_id.map(str::trim).filter(|s| !s.is_empty()) {
            q.after = whole("Last-Event-ID", id).map_err(bad)?;
        }
        Ok(q)
    }
}

/// A whole number; one too big for 64 bits is the biggest one.
fn whole(name: &str, v: &str) -> Result<u64, String> {
    if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{name} must be a whole number, not {v:?}"));
    }
    Ok(v.parse().unwrap_or(u64::MAX))
}

/// A query string's pairs, percent-decoded; `None` when a value is not UTF-8.
pub fn query_pairs(query: Option<&str>) -> Option<Vec<(String, String)>> {
    fn decode(s: &str) -> Option<String> {
        let b = s.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' && b.len() >= i + 3 {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8(out).ok()
    }
    query
        .unwrap_or("")
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            Some((decode(k)?, decode(v)?))
        })
        .collect()
}

/// The headers of an event-stream answer. `no-cache` and `no-transform` keep a
/// proxy from holding frames back.
pub fn sse_headers() -> Vec<(String, String)> {
    vec![
        ("content-type".into(), "text/event-stream".into()),
        ("cache-control".into(), "no-cache, no-transform".into()),
        ("x-accel-buffering".into(), "no".into()),
    ]
}

/// A stream written to a client: each frame, as the client takes it.
impl rung_agent_core::acp_http::BodyStream for Events {
    fn next(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Vec<u8>>> + Send + '_>> {
        Box::pin(async move { Events::next(self).await.map(|f| f.sse().into_bytes()) })
    }
}
