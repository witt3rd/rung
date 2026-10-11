//! H5: the event stream with replay, as a library (no HTTP).
//!
//! The record is written by hand into a state directory, a "running" host is
//! one whose state lock a test holds, and the stream is woken through the
//! bus or by the poll. No test sleeps: a wait is a wait for a frame, bounded
//! by a guard timeout that only a hang reaches. A sentinel is never put in an
//! assertion message: a failure says which frame kind leaked, not what.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rung_host::read::DoorError;
use rung_host::redact::Redactor;
use rung_host::statelock::StateLock;
use rung_host::stream::{
    Events, Frame, LiveBus, Options, Partial, PartialRedactor, Partials, What,
};
use rung_testkit::TempDir;
use serde_json::{Value, json};

/// A shapeless secret: found only because the redactor was told its value.
const PLAIN: &str = "plainsentinelh5-0123456789";
/// A secret with a known key shape.
const SHAPED: &str = "sk-SENTINELh5shaped0123456789abcdef";
/// Far longer than any test lasts: only the bus wakes a stream.
const NEVER: Duration = Duration::from_secs(3600);
/// A hang, not a delay.
const GUARD: Duration = Duration::from_secs(20);

fn redactor() -> Redactor {
    Redactor::new().with_secret(PLAIN)
}

struct Instance {
    dir: TempDir,
    lock: Option<StateLock>,
    bus: Arc<LiveBus>,
    next: u64,
    segment: u32,
}

impl Instance {
    fn new(tag: &str) -> Self {
        let dir = TempDir::new(tag);
        std::fs::create_dir_all(dir.path().join("record")).unwrap();
        Self {
            dir,
            lock: None,
            bus: LiveBus::new(),
            next: 1,
            segment: 1,
        }
    }

    fn state(&self) -> &Path {
        self.dir.path()
    }

    fn seg(&self) -> PathBuf {
        self.state()
            .join("record")
            .join(format!("seg-{:08}.ndjson", self.segment))
    }

    /// The host is alive: its lock is held.
    fn live(&mut self) {
        self.lock = Some(StateLock::acquire(self.state()).unwrap());
    }

    fn die(&mut self) {
        self.lock = None;
    }

    fn raw(&self, text: &str) {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.seg())
            .unwrap();
        f.write_all(text.as_bytes()).unwrap();
    }

    fn line_text(&mut self, kind: &str, body: Value) -> String {
        let mut m = body.as_object().cloned().unwrap_or_default();
        m.insert("seq".into(), self.next.into());
        m.insert("at".into(), (1_000 + self.next).into());
        m.insert("kind".into(), kind.into());
        self.next += 1;
        format!("{}\n", Value::Object(m))
    }

    /// Append one line and wake the streams.
    fn push(&mut self, kind: &str, body: Value) -> u64 {
        let t = self.line_text(kind, body);
        self.raw(&t);
        self.bus.wake();
        self.next - 1
    }

    fn push_n(&mut self, n: u64) {
        for i in 0..n {
            self.push("note.written", json!({"text": format!("note {i}")}));
        }
    }

    fn rotate(&mut self) {
        self.segment += 1;
    }

    fn opts(&self, after: u64) -> Options {
        let mut o = Options::new(redactor());
        o.after = after;
        o.bus = Some(self.bus.clone());
        o.poll = NEVER;
        o
    }

    async fn open(&self, after: u64) -> Events {
        Events::open(self.state(), self.opts(after)).await.unwrap()
    }

    fn record_bytes(&self) -> Vec<u8> {
        let mut all = Vec::new();
        for s in 1..=self.segment {
            let p = self
                .state()
                .join("record")
                .join(format!("seg-{s:08}.ndjson"));
            all.extend(std::fs::read(p).unwrap_or_default());
        }
        all
    }
}

async fn next(ev: &mut Events) -> Frame {
    tokio::time::timeout(GUARD, ev.next())
        .await
        .expect("the stream hung")
        .expect("the stream ended")
}

/// The record number of a record frame.
fn seq(f: &Frame) -> u64 {
    match f {
        Frame::Record(v) => v["seq"].as_u64().unwrap(),
        other => panic!("not a record frame: {other:?}"),
    }
}

async fn records(ev: &mut Events, n: usize) -> Vec<u64> {
    let mut out = Vec::new();
    while out.len() < n {
        match next(ev).await {
            Frame::Record(v) => out.push(v["seq"].as_u64().unwrap()),
            Frame::Open { .. } | Frame::Ping => {}
            other => panic!("unexpected frame: {other:?}"),
        }
    }
    out
}

/// Every frame to the end.
async fn drain(ev: &mut Events) -> Vec<Frame> {
    let mut out = Vec::new();
    while let Some(f) = tokio::time::timeout(GUARD, ev.next())
        .await
        .expect("the stream hung")
    {
        out.push(f);
    }
    out
}

fn partial(what: What) -> Partial {
    Partial {
        turn: 1,
        call: 0,
        what,
    }
}

// ─── replay and live ─────────────────────────────────────────────────────────

#[tokio::test]
async fn the_first_frame_says_where_the_stream_starts() {
    let mut i = Instance::new("h5-open");
    i.live();
    i.push_n(2);
    let mut ev = i.open(1).await;
    match next(&mut ev).await {
        Frame::Open {
            after,
            live,
            partials,
            ..
        } => {
            assert_eq!(after, 1);
            assert!(live);
            assert!(partials);
        }
        other => panic!("not open: {other:?}"),
    }
}

#[tokio::test]
async fn replay_then_live_in_order_with_no_gap_and_no_repeat() {
    let mut i = Instance::new("h5-live");
    i.live();
    i.push_n(5);
    let mut ev = i.open(0).await;
    assert_eq!(records(&mut ev, 5).await, [1, 2, 3, 4, 5]);
    // now live: each new line arrives once, in order
    i.push_n(3);
    assert_eq!(records(&mut ev, 3).await, [6, 7, 8]);
}

#[tokio::test]
async fn a_line_appended_during_the_replay_is_neither_missed_nor_repeated() {
    let mut i = Instance::new("h5-seam");
    i.live();
    i.push_n(4);
    let mut ev = i.open(0).await;
    assert_eq!(records(&mut ev, 2).await, [1, 2]);
    // mid-replay: two more lines land
    i.push_n(2);
    assert_eq!(records(&mut ev, 4).await, [3, 4, 5, 6]);
    // and the live part continues from exactly there
    i.push_n(1);
    assert_eq!(records(&mut ev, 1).await, [7]);
}

#[tokio::test]
async fn a_line_written_between_two_reads_is_not_lost_with_no_poll_to_save_it() {
    // The poll is an hour: only the wake can find a line, so a wake that fell
    // between "nothing more" and "waiting" would hang here.
    let mut i = Instance::new("h5-race");
    i.live();
    i.push_n(1);
    let mut ev = i.open(0).await;
    assert_eq!(records(&mut ev, 1).await, [1]);
    for round in 0..50u64 {
        i.push_n(1);
        assert_eq!(records(&mut ev, 1).await, [2 + round]);
    }
}

#[tokio::test]
async fn a_reconnect_resumes_after_the_cursor() {
    let mut i = Instance::new("h5-cursor");
    i.live();
    i.push_n(6);
    let mut a = i.open(0).await;
    assert_eq!(records(&mut a, 3).await, [1, 2, 3]);
    drop(a); // the client is cut after seeing 3
    i.push_n(2);
    let mut b = i.open(3).await;
    assert_eq!(records(&mut b, 5).await, [4, 5, 6, 7, 8]);
}

#[tokio::test]
async fn the_cursor_works_across_segments_and_the_stream_follows_a_rotation() {
    let mut i = Instance::new("h5-seg");
    i.live();
    i.push_n(3);
    i.rotate();
    i.push_n(3);
    i.rotate();
    i.push_n(2);
    let mut ev = i.open(4).await;
    assert_eq!(records(&mut ev, 4).await, [5, 6, 7, 8]);
    i.rotate(); // the host rolls to a new segment while we wait
    i.push_n(2);
    assert_eq!(records(&mut ev, 2).await, [9, 10]);
    // a full replay crosses every boundary too
    let mut all = i.open(0).await;
    assert_eq!(records(&mut all, 10).await, (1..=10).collect::<Vec<_>>());
}

#[tokio::test]
async fn a_cursor_ahead_of_the_record_is_refused_before_the_stream_starts() {
    let mut i = Instance::new("h5-ahead");
    i.live();
    i.push_n(3);
    let e: DoorError = match Events::open(i.state(), i.opts(9)).await {
        Err(e) => e,
        Ok(_) => panic!("a cursor past the end opened a stream"),
    };
    assert_eq!((e.status, e.code), (409, "cursor_ahead"));
    // at the end exactly is fine: nothing to replay, then live
    let mut ev = i.open(3).await;
    i.push_n(1);
    assert_eq!(records(&mut ev, 1).await, [4]);
}

#[tokio::test]
async fn a_missing_state_directory_is_refused() {
    let i = Instance::new("h5-missing");
    let gone = i.state().join("not-there");
    let e = match Events::open(&gone, i.opts(0)).await {
        Err(e) => e,
        Ok(_) => panic!("opened a stream on nothing"),
    };
    assert_eq!((e.status, e.code), (404, "state_dir_missing"));
}

#[tokio::test]
async fn a_line_still_being_written_is_not_sent_until_it_is_whole() {
    let mut i = Instance::new("h5-torn");
    i.live();
    i.push_n(1);
    let mut ev = i.open(0).await;
    assert_eq!(records(&mut ev, 1).await, [1]);
    let whole = i.line_text("note.written", json!({"text": "second"}));
    let (a, b) = whole.split_at(whole.len() / 2);
    i.raw(a);
    i.bus.wake();
    // half a line: nothing comes out
    assert!(
        tokio::time::timeout(Duration::from_millis(150), ev.next())
            .await
            .is_err(),
        "a half-written line was sent"
    );
    i.raw(b);
    i.bus.wake();
    assert_eq!(records(&mut ev, 1).await, [2]);
}

#[tokio::test]
async fn no_line_is_cut_or_capped_however_long() {
    let mut i = Instance::new("h5-long");
    i.live();
    let big = "x".repeat(3_000_000);
    i.push("note.written", json!({"text": big}));
    let mut ev = i.open(0).await;
    match next(&mut ev).await {
        Frame::Open { .. } => {}
        other => panic!("{other:?}"),
    }
    match next(&mut ev).await {
        Frame::Record(v) => assert_eq!(v["text"].as_str().unwrap().len(), 3_000_000),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_damaged_record_is_sent_as_far_as_it_is_intact_then_ended() {
    let mut i = Instance::new("h5-damage");
    i.live();
    i.push_n(2);
    i.next = 9; // a gap
    i.push_n(1);
    let mut ev = i.open(0).await;
    let frames = drain(&mut ev).await;
    let recs: Vec<u64> = frames
        .iter()
        .filter(|f| matches!(f, Frame::Record(_)))
        .map(seq)
        .collect();
    assert_eq!(recs, [1, 2]);
    match frames.last().unwrap() {
        Frame::End {
            reason,
            last_seq,
            problem,
        } => {
            assert_eq!(reason, "damaged");
            assert_eq!(*last_seq, 2);
            assert!(problem.is_some());
        }
        other => panic!("{other:?}"),
    }
}

// ─── a stopped instance ──────────────────────────────────────────────────────

#[tokio::test]
async fn a_stopped_instance_replays_its_record_and_ends_cleanly() {
    let mut i = Instance::new("h5-stopped");
    i.push_n(3);
    i.push("halted", json!({"why": "stopped"}));
    let mut ev = i.open(0).await;
    let frames = drain(&mut ev).await;
    assert!(matches!(frames[0], Frame::Open { live: false, .. }));
    let recs: Vec<u64> = frames
        .iter()
        .filter(|f| matches!(f, Frame::Record(_)))
        .map(seq)
        .collect();
    assert_eq!(recs, [1, 2, 3, 4]);
    match frames.last().unwrap() {
        Frame::End {
            reason, last_seq, ..
        } => {
            assert_eq!(reason, "stopped");
            assert_eq!(*last_seq, 4);
        }
        other => panic!("{other:?}"),
    }
    assert!(ev.next().await.is_none(), "nothing follows the end");
}

#[tokio::test]
async fn a_dead_instance_ends_as_down_and_an_empty_one_has_nothing_to_replay() {
    let mut i = Instance::new("h5-down");
    i.push_n(2);
    let mut ev = i.open(0).await;
    let frames = drain(&mut ev).await;
    assert!(matches!(
        frames.last().unwrap(),
        Frame::End { reason, last_seq: 2, .. } if reason == "down"
    ));
    let empty = Instance::new("h5-empty");
    let mut ev = empty.open(0).await;
    let frames = drain(empty_events(&mut ev)).await;
    assert!(matches!(
        frames.last().unwrap(),
        Frame::End { last_seq: 0, .. }
    ));
}

fn empty_events(e: &mut Events) -> &mut Events {
    e
}

#[tokio::test]
async fn a_stream_open_when_the_host_stops_replays_the_end_and_closes() {
    let mut i = Instance::new("h5-stops-live");
    i.live();
    i.push_n(2);
    let mut ev = i.open(0).await;
    assert_eq!(records(&mut ev, 2).await, [1, 2]);
    i.push("halted", json!({"why": "stopped"}));
    i.die();
    i.bus.wake();
    assert_eq!(records(&mut ev, 1).await, [3]);
    match next(&mut ev).await {
        Frame::End {
            reason, last_seq, ..
        } => {
            assert_eq!(reason, "stopped");
            assert_eq!(last_seq, 3);
        }
        other => panic!("{other:?}"),
    }
    assert!(ev.next().await.is_none());
}

// ─── partial frames ──────────────────────────────────────────────────────────

fn delta(f: Frame) -> Value {
    match f {
        Frame::Delta(v) => v,
        other => panic!("not a delta: {other:?}"),
    }
}

#[tokio::test]
async fn partial_frames_reach_a_live_client_flagged_as_deltas_and_never_enter_the_record() {
    let mut i = Instance::new("h5-partial");
    i.live();
    i.push_n(1);
    let mut ev = i.open(0).await;
    assert_eq!(records(&mut ev, 1).await, [1]);
    let before = i.record_bytes();
    let turn = i.bus.turn(2);
    turn.block_start(0);
    turn.text(0, "Hello, ");
    turn.text(0, "world. ");
    turn.block_end(0, None);
    turn.tool_call(0, "call_1", "ws_read");
    let start = delta(next(&mut ev).await);
    assert_eq!(start["kind"], "block_start");
    let a = delta(next(&mut ev).await);
    assert_eq!(a["kind"], "text");
    assert_eq!(a["turn"], 2);
    assert_eq!(a["call"], 0);
    let mut text = a["text"].as_str().unwrap().to_string();
    let mut kinds = vec!["text"];
    loop {
        let d = delta(next(&mut ev).await);
        let k = d["kind"].as_str().unwrap().to_string();
        if k == "text" {
            text.push_str(d["text"].as_str().unwrap());
        } else {
            kinds.push(if k == "tool_call" {
                "tool_call"
            } else {
                "other"
            });
            if k == "tool_call" {
                break;
            }
        }
    }
    assert_eq!(text, "Hello, world. ");
    assert!(kinds.contains(&"tool_call"));
    assert_eq!(i.record_bytes(), before, "a partial frame wrote the record");
}

#[tokio::test]
async fn partial_frames_are_not_replayed_to_a_client_that_connects_later() {
    let mut i = Instance::new("h5-noreplay");
    i.live();
    i.push_n(2);
    let turn = i.bus.turn(1);
    turn.block_start(0);
    turn.text(0, "said before you came. ");
    turn.block_end(0, None);
    let mut late = i.open(0).await;
    assert_eq!(records(&mut late, 2).await, [1, 2]);
    // the next thing it sees is what happens after it arrived
    let turn = i.bus.turn(2);
    turn.block_start(0);
    turn.text(0, "said after. ");
    turn.block_end(0, None);
    assert_eq!(delta(next(&mut late).await)["kind"], "block_start");
    let d = delta(next(&mut late).await);
    assert_eq!(d["turn"], 2);
    assert_eq!(d["text"], "said after. ");
}

#[tokio::test]
async fn partial_frames_of_every_kind_carry_their_fields() {
    let mut i = Instance::new("h5-kinds");
    i.live();
    let mut ev = i.open(0).await;
    next(&mut ev).await; // open
    let turn = i.bus.turn(5);
    turn.tool_call(1, "c1", "ws_write");
    turn.tool_input(1, "c1", "{\"path\":");
    turn.tool_input(1, "c1", " \"a.txt\"}");
    turn.block_end(1, Some("c1"));
    turn.tool_run(1, "ws_write", &json!({"path": "a.txt"}));
    turn.tool_done(1, "ws_write", true, "wrote a.txt");
    turn.end();
    let mut got = Vec::new();
    let mut input = String::new();
    while got
        .last()
        .map(|d: &Value| d["kind"] != "turn_end")
        .unwrap_or(true)
    {
        let d = delta(next(&mut ev).await);
        if d["kind"] == "tool_input" {
            input.push_str(d["delta"].as_str().unwrap());
        }
        got.push(d);
    }
    let kinds: Vec<&str> = got.iter().map(|d| d["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds.first(), Some(&"tool_call"));
    assert_eq!(kinds.last(), Some(&"turn_end"));
    assert!(kinds.contains(&"tool_run") && kinds.contains(&"tool_done"));
    assert_eq!(input, "{\"path\": \"a.txt\"}");
    let done = got.iter().find(|d| d["kind"] == "tool_done").unwrap();
    assert_eq!(done["ok"], true);
    assert_eq!(done["output"], "wrote a.txt");
    assert_eq!(done["call"], 1);
    let run = got.iter().find(|d| d["kind"] == "tool_run").unwrap();
    assert_eq!(run["input"], json!({"path": "a.txt"}));
}

#[tokio::test]
async fn off_and_only_choose_which_frames_a_stream_carries() {
    let mut i = Instance::new("h5-modes");
    i.live();
    i.push_n(2);
    // off: records only, and the open frame says so
    let mut o = i.opts(0);
    o.partials = Partials::Off;
    let mut off = Events::open(i.state(), o).await.unwrap();
    assert!(matches!(
        next(&mut off).await,
        Frame::Open {
            partials: false,
            ..
        }
    ));
    assert_eq!(records(&mut off, 2).await, [1, 2]);
    i.bus.turn(1).block_start(0);
    i.bus.turn(1).text(0, "ignored ");
    i.push_n(1);
    assert_eq!(records(&mut off, 1).await, [3], "off sent a delta");
    // only: partials, and no record is read
    let mut o = i.opts(0);
    o.partials = Partials::Only;
    let mut only = Events::open(i.state(), o).await.unwrap();
    assert!(matches!(next(&mut only).await, Frame::Open { .. }));
    i.bus.turn(1).block_start(0);
    i.bus.turn(1).text(0, "seen ");
    i.bus.turn(1).block_end(0, None);
    assert_eq!(delta(next(&mut only).await)["kind"], "block_start");
    let d = delta(next(&mut only).await);
    assert_eq!(d["text"], "seen ");
}

#[tokio::test]
async fn partial_frames_from_upstream_are_relayed_like_the_hosts_own() {
    let mut i = Instance::new("h5-upstream");
    i.live();
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let mut o = i.opts(0);
    o.bus = None;
    o.upstream = Some(rx);
    let mut ev = Events::open(i.state(), o).await.unwrap();
    assert!(matches!(
        next(&mut ev).await,
        Frame::Open { partials: true, .. }
    ));
    tx.send(partial(What::BlockStart { id: None }))
        .await
        .unwrap();
    tx.send(partial(What::Text {
        text: "from the host ".into(),
    }))
    .await
    .unwrap();
    tx.send(partial(What::BlockEnd { id: None })).await.unwrap();
    assert_eq!(delta(next(&mut ev).await)["kind"], "block_start");
    let d = delta(next(&mut ev).await);
    assert_eq!(d["text"], "from the host ");
}

// ─── redaction on every frame ────────────────────────────────────────────────

/// The whole of an SSE stream's text for the frames, to look for a leak.
fn leaked(frames: &[Frame], secret: &str) -> Vec<&'static str> {
    frames
        .iter()
        .filter(|f| f.sse().contains(secret))
        .map(|f| match f {
            Frame::Open { .. } => "open",
            Frame::Record(_) => "record",
            Frame::Delta(_) => "delta",
            Frame::Ping => "ping",
            Frame::End { .. } => "end",
        })
        .collect()
}

#[tokio::test]
async fn a_secret_in_a_record_line_never_appears_in_a_frame() {
    let mut i = Instance::new("h5-red-record");
    i.live();
    i.push(
        "note.written",
        json!({"text": format!("the key is {PLAIN} and {SHAPED}"), "n": 1}),
    );
    i.push(
        "turn.ended",
        json!({"turn": 1, "final_text": format!("done {PLAIN}"), "tools": [{"out": SHAPED}]}),
    );
    let mut ev = i.open(0).await;
    next(&mut ev).await;
    let a = next(&mut ev).await;
    let b = next(&mut ev).await;
    assert!(a.sse().contains("the key is"), "the line is still sent");
    for s in [PLAIN, SHAPED] {
        assert!(
            leaked(&[a.clone(), b.clone()], s).is_empty(),
            "a record leaked"
        );
    }
    // the record on disk stays verbatim
    assert!(String::from_utf8(i.record_bytes()).unwrap().contains(PLAIN));
}

#[tokio::test]
async fn a_secret_in_every_kind_of_partial_frame_never_appears() {
    let mut i = Instance::new("h5-red-partial");
    i.live();
    let mut ev = i.open(0).await;
    next(&mut ev).await;
    let t = i.bus.turn(1);
    t.block_start(0);
    t.text(0, &format!("my key is {PLAIN} ok. "));
    t.text(0, &format!("and {SHAPED} too. "));
    t.block_end(0, None);
    t.tool_call(0, &format!("id-{PLAIN}"), &format!("tool {SHAPED}"));
    t.tool_call(0, "c", "shell");
    t.tool_input(0, "c", &format!("{{\"token\": \"{PLAIN}\"}}"));
    t.block_end(0, Some("c"));
    t.tool_run(
        0,
        "ws_write",
        &json!({"path": "k", "text": PLAIN, SHAPED: 1}),
    );
    t.tool_done(
        0,
        "ws_write",
        false,
        &format!("error with {SHAPED} and {PLAIN}"),
    );
    t.end();
    let mut frames = Vec::new();
    loop {
        let f = next(&mut ev).await;
        let done = matches!(&f, Frame::Delta(v) if v["kind"] == "turn_end");
        frames.push(f);
        if done {
            break;
        }
    }
    for s in [PLAIN, SHAPED] {
        let kinds = leaked(&frames, s);
        let which: Vec<String> = frames
            .iter()
            .filter(|f| f.sse().contains(s))
            .map(|f| match f {
                Frame::Delta(v) => v["kind"].to_string(),
                _ => "other".into(),
            })
            .collect();
        assert!(kinds.is_empty(), "a secret leaked in a {which:?} frame");
    }
    // and the rest of each frame was kept
    let all: String = frames.iter().map(Frame::sse).collect();
    assert!(all.contains("my key is") && all.contains("ws_write"));
}

#[tokio::test]
async fn an_error_text_and_the_end_frame_carry_no_secret() {
    let mut i = Instance::new("h5-red-end");
    i.live();
    i.push_n(1);
    i.next = 5;
    i.push("note.written", json!({"text": PLAIN}));
    // the damage problem names a file under the state directory
    let mut o = i.opts(0);
    o.red = Redactor::new()
        .with_secret(PLAIN)
        .with_secret(i.state().to_str().unwrap());
    let mut ev = Events::open(i.state(), o).await.unwrap();
    let frames = drain(&mut ev).await;
    assert!(leaked(&frames, i.state().to_str().unwrap()).is_empty());
    assert!(leaked(&frames, PLAIN).is_empty());
}

/// All the text a redactor lets out for `pieces`, flushed at the end.
fn through(red: &Redactor, pieces: &[&str]) -> String {
    let mut pr = PartialRedactor::new(red.clone());
    let mut out = String::new();
    let mut take = |frames: Vec<Value>| {
        for f in frames {
            if f["kind"] == "text" {
                out.push_str(f["text"].as_str().unwrap());
            }
        }
    };
    take(pr.push(partial(What::BlockStart { id: None })));
    for p in pieces {
        take(pr.push(partial(What::Text {
            text: (*p).to_string(),
        })));
    }
    take(pr.push(partial(What::BlockEnd { id: None })));
    out
}

#[test]
fn a_secret_split_between_two_pieces_is_still_removed() {
    let red = redactor();
    let sentences = [
        format!("here it is: {PLAIN}, keep it safe. And {SHAPED}!"),
        PLAIN.to_string(),
        format!("see {SHAPED}"),
        "Authorization: Bearer abcdefghij1234567890 trailing words".to_string(),
        "export API_KEY=hunter2hunter2 && go".to_string(),
        "no secret at all, just words and numbers 12345.".to_string(),
        "multi\nline text\nwith password: swordfish99 here\nand more".to_string(),
    ];
    for text in &sentences {
        let want = red.redact(text).into_owned();
        // every split into two pieces, at every character boundary
        for (at, _) in text.char_indices().chain([(text.len(), ' ')]) {
            let got = through(&red, &[&text[..at], &text[at..]]);
            assert!(got == want, "a two-piece split changed the redaction");
        }
        // and one character at a time
        let pieces: Vec<String> = text.chars().map(String::from).collect();
        let refs: Vec<&str> = pieces.iter().map(String::as_str).collect();
        assert!(
            through(&red, &refs) == want,
            "piecewise changed the redaction"
        );
    }
}

#[test]
fn a_private_key_block_split_across_pieces_is_removed() {
    let red = redactor();
    let text = "before\n-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\nabcDEF123456\n-----END PRIVATE KEY-----\nafter";
    let want = red.redact(text).into_owned();
    assert!(!want.contains("MIIEvQ"), "the private key block leaked");
    let pieces: Vec<String> = text.chars().map(String::from).collect();
    let refs: Vec<&str> = pieces.iter().map(String::as_str).collect();
    let got = through(&red, &refs);
    assert!(got == want, "pieces changed the redaction of a key block");
}

#[test]
fn text_is_released_as_it_forms_not_only_at_the_end() {
    let red = redactor();
    let mut pr = PartialRedactor::new(red);
    pr.push(partial(What::BlockStart { id: None }));
    let text = "The quick brown fox jumps over the lazy dog and keeps running far away ";
    let first = pr.push(partial(What::Text { text: text.into() }));
    let said: String = first
        .iter()
        .filter_map(|f| f["text"].as_str())
        .collect::<String>();
    // all but the last few bytes (the longest secret's length) are out
    assert!(
        said.len() + PLAIN.len() + 12 >= text.len() && text.starts_with(&said),
        "text was held longer than it must be: {said:?}"
    );
}

#[test]
fn the_end_of_a_block_or_a_turn_releases_what_was_held() {
    let red = redactor();
    let mut pr = PartialRedactor::new(red.clone());
    pr.push(partial(What::BlockStart { id: None }));
    let a = pr.push(partial(What::Text {
        text: "tail-of-a-word".into(),
    }));
    let held: String = a.iter().filter_map(|f| f["text"].as_str()).collect();
    assert_ne!(held, "tail-of-a-word", "an unfinished word was released");
    let b = pr.push(partial(What::TurnEnd));
    let rest: String = b.iter().filter_map(|f| f["text"].as_str()).collect();
    assert_eq!(format!("{held}{rest}"), "tail-of-a-word");
    assert_eq!(b.last().unwrap()["kind"], "turn_end");
}

// ─── slow clients, many clients, disconnects ─────────────────────────────────

#[tokio::test]
async fn a_client_that_falls_behind_on_partials_is_told_and_loses_no_record_line() {
    let mut i = Instance::new("h5-slow");
    i.live();
    i.push_n(3);
    let mut slow = i.open(0).await;
    next(&mut slow).await; // open
    // the host publishes far more than a subscriber may hold
    let turn = i.bus.turn(1);
    turn.block_start(0);
    for n in 0..10_000u32 {
        turn.text(0, &format!("w{n} "));
    }
    turn.block_end(0, None);
    // the next block begins after the gap, so the client sees it whole
    turn.block_start(1);
    turn.text(1, "words of the next block ");
    turn.block_end(1, None);
    i.push_n(2);
    let mut behind = None;
    let mut recs = Vec::new();
    let mut texts = 0u64;
    while recs.len() < 5 || behind.is_none() || texts == 0 {
        match next(&mut slow).await {
            Frame::Record(v) => recs.push(v["seq"].as_u64().unwrap()),
            Frame::Delta(v) if v["kind"] == "behind" => behind = Some(v),
            Frame::Delta(v) if v["kind"] == "text" => texts += 1,
            Frame::Delta(_) | Frame::Open { .. } | Frame::Ping => {}
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(recs, [1, 2, 3, 4, 5], "a record line was lost");
    let behind = behind.expect("a lagging client was not told it fell behind");
    assert!(behind["skipped"].as_u64().unwrap() > 0);
    assert!(texts > 0, "the stream did not resume at the next block");
}

#[tokio::test]
async fn many_clients_each_get_the_whole_record_and_do_not_block_each_other() {
    let mut i = Instance::new("h5-many");
    i.live();
    i.push_n(20);
    let mut clients = Vec::new();
    for _ in 0..100 {
        clients.push(i.open(0).await);
    }
    // client 0 never reads again; the others do not wait for it
    let stalled = clients.remove(0);
    let mut tasks = Vec::new();
    for mut c in clients {
        tasks.push(tokio::spawn(async move {
            let got = records(&mut c, 20).await;
            let more = records(&mut c, 5).await;
            (got, more)
        }));
    }
    i.push_n(5);
    for t in tasks {
        let (got, more) = tokio::time::timeout(GUARD, t).await.unwrap().unwrap();
        assert_eq!(got, (1..=20).collect::<Vec<_>>());
        assert_eq!(more, (21..=25).collect::<Vec<_>>());
    }
    drop(stalled);
}

#[test]
fn waiting_clients_hold_no_blocking_thread() {
    // One blocking thread in the whole runtime: if a waiting client held one,
    // the second client (and the live line) would never be served.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut i = Instance::new("h5-pool");
        i.live();
        i.push_n(2);
        let mut clients = Vec::new();
        for _ in 0..40 {
            clients.push(i.open(0).await);
        }
        for c in clients.iter_mut() {
            assert_eq!(records(c, 2).await, [1, 2]);
        }
        // all forty are now waiting for a line; one arrives
        i.push_n(1);
        for c in clients.iter_mut() {
            assert_eq!(records(c, 1).await, [3]);
        }
    });
}

#[tokio::test]
async fn a_client_that_goes_away_frees_its_subscription() {
    let mut i = Instance::new("h5-free");
    i.live();
    i.push_n(1);
    assert_eq!(i.bus.subscribers(), 0);
    let mut a = i.open(0).await;
    let b = i.open(0).await;
    assert_eq!(i.bus.subscribers(), 2);
    next(&mut a).await;
    drop(a);
    assert_eq!(i.bus.subscribers(), 1);
    drop(b);
    assert_eq!(i.bus.subscribers(), 0);
    // a stream that ended frees it too
    let mut c = i.open(0).await;
    i.push("halted", json!({}));
    i.die();
    let _ = drain(&mut c).await;
    assert_eq!(
        i.bus.subscribers(),
        0,
        "an ended stream kept its subscription"
    );
}

#[tokio::test]
async fn an_idle_stream_writes_a_keepalive() {
    let mut i = Instance::new("h5-ping");
    i.live();
    let mut o = i.opts(0);
    o.keepalive = Duration::from_millis(20);
    let mut ev = Events::open(i.state(), o).await.unwrap();
    assert!(matches!(next(&mut ev).await, Frame::Open { .. }));
    assert_eq!(next(&mut ev).await, Frame::Ping);
    assert_eq!(Frame::Ping.sse(), ": keepalive\n\n");
}

#[test]
fn frames_are_server_sent_events_and_only_record_frames_have_an_id() {
    let rec = Frame::Record(json!({"seq": 7, "at": 1, "kind": "k"}));
    let s = rec.sse();
    assert!(s.starts_with("event: record\nid: 7\ndata: {"));
    assert!(s.ends_with("\n\n"));
    assert_eq!(s.matches('\n').count(), 4, "one data line: {s:?}");
    for f in [
        Frame::Delta(json!({"kind": "text", "text": "a\nb"})),
        Frame::Open {
            after: 0,
            live: true,
            partials: true,
            env_unset: vec![],
        },
        Frame::End {
            reason: "stopped".into(),
            last_seq: 1,
            problem: None,
        },
    ] {
        let s = f.sse();
        assert!(!s.contains("\nid:"), "{s:?}");
        // a newline inside the payload is escaped, so the frame is one data line
        assert_eq!(s.matches("data:").count(), 1);
        assert!(
            s.trim_end()
                .lines()
                .all(|l| l.starts_with("event:") || l.starts_with("data:"))
        );
    }
}

// ─── redaction across a gap and across a join ────────────────────────────────

/// What a client sees of the text of the answer and the whole wire, for
/// checks that name no secret.
struct Seen {
    text: String,
    wire: String,
    behind: bool,
}

/// Frames up to the turn's end, for a client at `ev`.
async fn until_turn_end(ev: &mut Events) -> Seen {
    let mut seen = Seen {
        text: String::new(),
        wire: String::new(),
        behind: false,
    };
    loop {
        let f = next(ev).await;
        seen.wire.push_str(&f.sse());
        if let Frame::Delta(v) = &f {
            match v["kind"].as_str() {
                Some("text") => seen.text.push_str(v["text"].as_str().unwrap()),
                Some("behind") => seen.behind = true,
                Some("turn_end") => return seen,
                _ => {}
            }
        }
    }
}

/// Pieces of the sentinel: no piece of this length of it may reach a client.
fn fragments() -> [&'static str; 2] {
    [&PLAIN[..10], &PLAIN[13..]]
}

fn no_fragment(seen: &Seen, what: &str) {
    for f in fragments() {
        assert!(
            !seen.wire.contains(f),
            "a piece of a secret reached a client: {what}"
        );
    }
    assert!(
        !seen.wire.contains(PLAIN),
        "a secret reached a client: {what}"
    );
}

#[tokio::test]
async fn a_secret_split_across_the_frames_a_client_missed_never_appears() {
    let mut i = Instance::new("h5-gap");
    i.live();
    let mut ev = i.open(0).await;
    next(&mut ev).await; // open
    let t = i.bus.turn(1);
    // the client has seen the start of the answer and the first part of the key
    t.block_start(0);
    t.text(
        0,
        "Preamble words, so that some text has gone out already. ",
    );
    t.text(0, &format!("My key is {}", &PLAIN[..12]));
    t.tool_done(1, "marker", true, "m");
    loop {
        // read up to the marker: the redactor has seen everything so far
        if let Frame::Delta(v) = next(&mut ev).await
            && v["kind"] == "tool_done"
        {
            break;
        }
    }
    // then it reads too slowly: far more frames than it may hold are published
    for n in 0..6_000u32 {
        t.tool_done(2, "filler", true, &n.to_string());
    }
    t.text(0, &format!("{} and the words after it. ", &PLAIN[12..]));
    t.block_end(0, None);
    // a later block begins cleanly
    t.block_start(1);
    t.text(1, "Fresh words of a new block. ");
    t.block_end(1, None);
    t.end();
    let seen = until_turn_end(&mut ev).await;
    no_fragment(&seen, "across the frames of a gap");
    assert!(seen.behind, "the client was not told it fell behind");
    assert!(
        seen.text.contains("Fresh words of a new block."),
        "the stream did not resume at the next block"
    );
    assert!(
        !seen.text.contains("and the words after it."),
        "the rest of a block that was cut by a gap was sent"
    );
}

#[tokio::test]
async fn a_secret_split_at_a_gap_when_the_client_had_held_text_never_appears() {
    // the same, with the first part of the key held back by the redactor (the
    // last word is always held) at the moment of the gap
    let mut i = Instance::new("h5-gap-held");
    i.live();
    let mut ev = i.open(0).await;
    next(&mut ev).await;
    let t = i.bus.turn(1);
    t.block_start(0);
    t.text(0, &format!("key {}", &PLAIN[..20]));
    t.tool_done(1, "marker", true, "m");
    loop {
        if let Frame::Delta(v) = next(&mut ev).await
            && v["kind"] == "tool_done"
        {
            break;
        }
    }
    for n in 0..6_000u32 {
        t.tool_done(2, "filler", true, &n.to_string());
    }
    t.text(0, &format!("{} end. ", &PLAIN[20..]));
    t.block_end(0, None);
    t.end();
    let seen = until_turn_end(&mut ev).await;
    no_fragment(&seen, "held across a gap");
    assert!(seen.behind);
}

#[tokio::test]
async fn a_client_joining_in_the_middle_of_a_block_never_sees_a_secret_split_across_the_join() {
    let mut i = Instance::new("h5-join");
    i.live();
    let t = i.bus.turn(1);
    // the block begins, and the first part of the key goes out, before the client
    t.block_start(0);
    t.text(
        0,
        &format!("Words before you came, my key is {}", &PLAIN[..12]),
    );
    let mut ev = i.open(0).await;
    next(&mut ev).await; // open
    t.text(0, &format!("{} and more words. ", &PLAIN[12..]));
    t.block_end(0, None);
    t.block_start(1);
    t.text(1, "A block that began after you came. ");
    t.block_end(1, None);
    t.end();
    let seen = until_turn_end(&mut ev).await;
    no_fragment(&seen, "across a mid-block join");
    assert!(
        seen.text.contains("A block that began after you came."),
        "blocks that begin after the join are sent"
    );
    assert!(
        !seen.text.contains("and more words."),
        "a block joined in the middle was sent"
    );
}

#[test]
fn a_gap_discards_what_was_held_and_the_text_of_blocks_that_were_open() {
    let red = redactor();
    let mut pr = PartialRedactor::new(red);
    pr.push(partial(What::BlockStart { id: None }));
    pr.push(partial(What::Text {
        text: format!("Some words and then {}", &PLAIN[..14]),
    }));
    let out = pr.push(partial(What::Behind { skipped: 3 }));
    let said: String = out.iter().filter_map(|f| f["text"].as_str()).collect();
    assert!(
        !said.contains(&PLAIN[..14]),
        "a held piece of a secret was released at a gap"
    );
    assert_eq!(out.last().unwrap()["kind"], "behind");
    // more of that block, and its end: nothing
    let a = pr.push(partial(What::Text {
        text: format!("{} more words. ", &PLAIN[14..]),
    }));
    assert!(a.is_empty());
    let b = pr.push(partial(What::BlockEnd { id: None }));
    assert!(
        b.is_empty(),
        "the end of a block the client never saw began"
    );
    // a block that begins after the gap is whole
    pr.push(partial(What::BlockStart { id: None }));
    let c = pr.push(partial(What::Text {
        text: "New words here, enough of them that most go out at once. ".into(),
    }));
    assert!(!c.is_empty());
}

#[test]
fn text_of_a_block_whose_start_was_not_seen_is_not_sent() {
    let mut pr = PartialRedactor::new(redactor());
    assert!(
        pr.push(partial(What::Text {
            text: "words from the middle of a block ".into()
        }))
        .is_empty()
    );
    assert!(
        pr.push(partial(What::ToolInput {
            id: "c9".into(),
            delta: "{\"a\": ".into()
        }))
        .is_empty()
    );
    // a tool call seen from its start is sent
    pr.push(partial(What::ToolCall {
        id: "c1".into(),
        name: "t".into(),
    }));
    let out = pr.push(partial(What::ToolInput {
        id: "c1".into(),
        delta: "{\"a\": 1, \"b\": \"enough text that most of it goes out at once\"} ".into(),
    }));
    assert!(!out.is_empty());
}
