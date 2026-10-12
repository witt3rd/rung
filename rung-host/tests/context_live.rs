//! S6 (host half): the layers of a turn's context are in the record from the
//! turn's start, and the context door answers for a turn that is still
//! running, whole, then settles at the final record. A sentinel secret is never
//! put in an assertion message.

mod common;

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::ctx::*;
use rung_host::canon;
use rung_host::engine::{EngineTurn, TurnEngine, TurnRequest};
use rung_host::memory::MemoryHost;
use rung_host::pack::request_bytes;
use rung_host::read;
use rung_host::redact::Redactor;
use rung_host::sim::{self, SIM_START};
use serde_json::{Value, json};

const CANARY: &str = "livecanarys6-4242-9a8b7c6d5e";

/// The scripted engine, held inside one turn until released: it says it is
/// there, remembers the request it was handed, and waits.
struct Hold {
    inner: Arc<Script>,
    at_turn: u64,
    entered: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
    held: Mutex<Option<Seen>>,
}

impl TurnEngine for Hold {
    fn name(&self) -> &str {
        "hold"
    }
    fn turn(&self, req: TurnRequest) -> EngineTurn {
        if req.turn == self.at_turn {
            *self.held.lock().unwrap() = Some(Seen {
                turn: req.turn,
                tools: req.tools.definitions(),
                system: req.thread.system_prompt.clone(),
                messages: req.thread.messages.clone(),
            });
            self.entered.lock().unwrap().send(()).unwrap();
            let _ = self
                .release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(120));
        }
        self.inner.turn(req)
    }
}

/// A host that runs `turns` turns of `calls` model calls and is held inside
/// the last one.
struct Held {
    guard: sim::TempDir,
    turns: u64,
    engine: Arc<Hold>,
    entered: Receiver<()>,
    release: Sender<()>,
    run: std::thread::JoinHandle<()>,
}

impl Held {
    fn start(tag: &str, calls: usize, turns: u64, tune: impl FnOnce(&mut sim::Scenario)) -> Self {
        sim::test_timeout(300);
        let guard = sim::temp_dir_guard(tag);
        let (mut sc, script) = scripted(guard.path(), calls, turns);
        let (etx, entered) = channel();
        let (release, rrx) = channel();
        let engine = Arc::new(Hold {
            inner: script,
            at_turn: turns,
            entered: Mutex::new(etx),
            release: Mutex::new(rrx),
            held: Mutex::new(None),
        });
        sc.engine = Some(engine.clone());
        tune(&mut sc);
        let (host, rec, _) = sim::build(sc);
        let run = std::thread::spawn(move || {
            let _ = host.run(rec);
        });
        Self {
            guard,
            turns,
            engine,
            entered,
            release,
            run,
        }
    }

    /// Wait until the engine is inside the held turn.
    fn wait_inside(&self) {
        self.entered
            .recv_timeout(Duration::from_secs(120))
            .expect("the held turn began");
    }

    fn dir(&self) -> &std::path::Path {
        self.guard.path()
    }

    /// Let the turn end and the run finish.
    fn finish(self) -> (sim::TempDir, Arc<Hold>) {
        let _ = self.release.send(());
        self.run.join().expect("the run ended");
        (self.guard, self.engine)
    }
}

/// The bands without their cache marks (a mark needs the provider's count,
/// which only an answered call has).
fn bands_without_marks(v: &Value) -> Vec<Value> {
    v["bands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            let mut b = b.clone();
            b.as_object_mut().unwrap().remove("cache");
            b
        })
        .collect()
}

/// The text of the request the bands rebuild, as the provider is sent it.
fn rebuilt(v: &Value) -> Vec<u8> {
    let defs: Vec<Value> = band(v, "tools")["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| serde_json::from_str(i["text"].as_str().unwrap()).unwrap())
        .collect();
    let mut msgs: Vec<Value> = vec![json!({"content": band(v, "epoch")["text"], "role": "user"})];
    for row in band(v, "history")["rows"].as_array().unwrap() {
        msgs.push(json!({"content": row["header"], "role": "user"}));
        msgs.extend(row["messages"].as_array().unwrap().iter().cloned());
    }
    msgs.push(json!({"content": band(v, "message")["text"], "role": "user"}));
    msgs.extend(
        band(v, "this_turn")["messages"]
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    let mut got = canon::bytes(&Value::Array(defs));
    got.push(b'\n');
    got.extend(canon::of(band(v, "system")["text"].as_str().unwrap()));
    for m in &msgs {
        got.push(b'\n');
        got.extend(canon::bytes(m));
    }
    got
}

// ─── the tests ───────────────────────────────────────────────────────────────

/// (1) A turn in progress exposes its layers through the door before it ends,
/// whole: the bands rebuild, byte for byte, the request the engine was handed.
#[test]
fn a_running_turn_shows_its_layers_through_the_door_whole() {
    let h = Held::start("live-whole", 3, 2, |_| {});
    h.wait_inside();
    let red = Redactor::new();
    let v = door(h.dir(), h.turns, None, &red).expect("the door answers for a running turn");
    assert_eq!(v["turn"], h.turns);
    assert_eq!(v["call"], 1);
    assert_eq!(v["live"], true);
    assert_eq!(v["calls"], 0, "no model call has been answered yet");
    assert_eq!(v["recorded"], true);
    assert!(v["provider"]["prompt_tokens"].is_null());
    let held = h.engine.held.lock().unwrap().clone().expect("held");
    let want = request_bytes(&held.tools, &held.system, &held.messages);
    assert!(
        rebuilt(&v) == want,
        "the bands rebuild the request being made"
    );
    assert_eq!(band(&v, "history")["turns"], h.turns - 1);
    assert_eq!(band(&v, "this_turn")["messages"], json!([]));
    // asking for a later request of a turn that has not ended is refused in
    // words, not answered with a guess
    assert_eq!(
        door(h.dir(), h.turns, Some(2), &red).unwrap_err().1,
        "turn_not_logged"
    );
    assert_eq!(
        door(h.dir(), h.turns, Some(0), &red).unwrap_err().1,
        "no_such_call"
    );
    // the layers are in the record as a line of their own
    let lines = record_lines(h.dir());
    let at = lines
        .iter()
        .position(|l| l.kind == "turn.layers" && l.u64("turn") == h.turns)
        .expect("a turn.layers line");
    assert_eq!(
        lines[at - 1].kind,
        "turn.started",
        "written with turn.started"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.kind == "turn.log" && l.u64("turn") == h.turns)
    );
    let _ = h.finish();
}

/// (2) The final record is what was shown mid-turn plus the outcome: no layer
/// changes meaning when the turn ends.
#[test]
fn the_final_record_is_what_was_shown_plus_the_outcome() {
    let h = Held::start("live-settle", 3, 3, |_| {});
    h.wait_inside();
    let red = Redactor::new();
    let mid = door(h.dir(), h.turns, Some(1), &red).unwrap();
    let turn = h.turns;
    let (dir, _) = h.finish();
    let fin = door(dir.path(), turn, Some(1), &red).unwrap();
    assert_eq!(fin["live"], false);
    assert_eq!(fin["calls"], 3);
    assert_eq!(fin["recorded"], true);
    assert!(
        fin["provider"]["prompt_tokens"].is_number(),
        "the outcome: the provider's count"
    );
    assert_eq!(
        bands_without_marks(&mid),
        bands_without_marks(&fin),
        "every layer reads the same before and after the turn ends"
    );
    assert!(rebuilt(&mid) == rebuilt(&fin));
    // the outcome is in the record, after the layers
    let lines = record_lines(dir.path());
    let layers = lines
        .iter()
        .position(|l| l.kind == "turn.layers" && l.u64("turn") == turn)
        .unwrap();
    let ended = lines
        .iter()
        .position(|l| l.kind == "turn.ended" && l.u64("turn") == turn)
        .unwrap();
    assert!(layers < ended);
    // the end-of-turn line carries the header it was shown with, unchanged, and
    // does not write the recall a second time
    let (l, log) = (
        &lines[layers],
        lines
            .iter()
            .find(|l| l.kind == "turn.log" && l.u64("turn") == turn)
            .unwrap(),
    );
    for k in ["header", "recall"] {
        assert_eq!(l.get(k), log.get(k), "{k}");
    }
    assert!(!log.body.contains_key("recall_text") && !log.body.contains_key("recall_hash"));
}

/// (3) A secret in a layer reads `[redacted]` mid-turn too, in the record line
/// itself and in the door's answer.
#[test]
fn a_secret_in_a_layer_is_redacted_mid_turn() {
    let h = Held::start("live-canary", 2, 1, |sc| {
        sc.redactor = Some(Redactor::new().with_secret(CANARY));
        sc.sources.push(owner_says(
            SIM_START,
            &format!("what is the code {CANARY} please"),
        ));
        sc.memory_host = Some(Arc::new(MemoryHost::new(
            Arc::new(Big {
                text: long_recall(300, Some((7, CANARY))),
            }),
            "s",
        )));
    });
    h.wait_inside();
    // the door's own redactor knows nothing: what is tested is the record
    let v = door(h.dir(), h.turns, None, &Redactor::new()).unwrap();
    let text = serde_json::to_string(&v).unwrap();
    assert!(
        !text.contains(CANARY),
        "the secret reached the door's answer"
    );
    assert!(
        band(&v, "message")["text"]
            .as_str()
            .unwrap()
            .contains("[redacted]")
    );
    assert!(
        band(&v, "recall")["text"]
            .as_str()
            .unwrap()
            .contains("[redacted]")
    );
    let lines = record_lines(h.dir());
    let l = lines.iter().find(|l| l.kind == "turn.layers").unwrap();
    assert!(
        !l.text().contains(CANARY),
        "the secret reached the turn.layers line"
    );
    assert!(l.str("header").contains("[redacted]"));
    assert!(l.str("recall_text").contains("[redacted]"));
    // and the record file, wherever the secret is not the owner's own message
    // kept verbatim as history, holds it nowhere in the layers
    let _ = h.finish();
}

/// A deterministic text of `lines` pieces of 100 bytes each, opened and closed by
/// markers: a layer nothing can cut without it showing. Single spaces only: the
/// header puts a message on one line, collapsing any other white space.
fn big_text(lines: usize, tag: &str) -> String {
    let mut s = format!("BEGIN-{tag} ");
    for i in 0..lines {
        s.push_str(&format!(
            "{tag}-{i:08} {} ",
            "k".repeat(100 - tag.len() - 12)
        ));
    }
    s.push_str(&format!("END-{tag}"));
    s
}

/// The part of `text` from `BEGIN-{tag}` to `END-{tag}`.
fn marked<'a>(text: &'a str, tag: &str) -> &'a str {
    let a = text
        .find(&format!("BEGIN-{tag}"))
        .expect("the start of the layer");
    let end = format!("END-{tag}");
    let b = a + text[a..].find(&end).expect("the end of the layer") + end.len();
    &text[a..b]
}

/// (4) A 20 MB layer arrives whole, mid-turn, with no cap: the hash of what the
/// door gives equals the hash of what was put in.
#[test]
fn a_20mb_layer_arrives_whole_mid_turn() {
    let body = big_text(220_000, "huge");
    assert!(body.len() >= 20 * 1024 * 1024);
    let want = canon::hash(body.as_bytes());
    let said = body.clone();
    let h = Held::start("live-huge", 2, 1, move |sc| {
        sc.sources.push(owner_says(SIM_START, &said));
    });
    h.wait_inside();
    let v = door(h.dir(), h.turns, None, &Redactor::new()).unwrap();
    let got = band(&v, "message")["text"].as_str().unwrap();
    let got = marked(got, "huge");
    assert_eq!(got.len(), body.len(), "the layer was cut");
    assert_eq!(canon::hash(got.as_bytes()), want);
    let lines = record_lines(h.dir());
    let l = lines.iter().find(|l| l.kind == "turn.layers").unwrap();
    assert_eq!(
        canon::hash(marked(l.str("header"), "huge").as_bytes()),
        want
    );
    let _ = h.finish();
}

/// (5) A reader that looks while the turn begins never sees a half-written
/// line: the record reads intact every time, and the door answers either with
/// a refusal in words or with the whole layer.
#[test]
fn a_reader_mid_turn_never_sees_a_half_written_line() {
    let body = big_text(40_000, "race");
    let want = canon::hash(body.as_bytes());
    let said = body.clone();
    let h = Held::start("live-race", 2, 1, move |sc| {
        sc.sources.push(owner_says(SIM_START, &said));
    });
    let dir = h.dir().to_path_buf();
    let turn = h.turns;
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let red = Redactor::new();
            let (mut whole, mut refused, mut looks) = (0u32, 0u32, 0u32);
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                looks += 1;
                let scanned = read::scan(&dir, |_| {}).expect("the record is readable");
                assert!(scanned.damaged.is_none(), "a reader saw a damaged record");
                match door(&dir, turn, None, &red) {
                    Ok(v) => {
                        let m = band(&v, "message")["text"].as_str().unwrap();
                        assert_eq!(
                            canon::hash(marked(m, "race").as_bytes()),
                            want,
                            "a reader saw a layer that is not whole"
                        );
                        whole += 1;
                    }
                    Err((status, code)) => {
                        assert!(
                            matches!(
                                (status, code.as_str()),
                                (404, "no_such_turn") | (409, "turn_not_logged")
                            ),
                            "a refusal that is not about a turn not begun: {status} {code}"
                        );
                        refused += 1;
                    }
                }
            }
            (looks, whole, refused)
        })
    };
    h.wait_inside();
    // the turn is held: give the reader time to look at the settled record
    std::thread::sleep(Duration::from_millis(300));
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let (looks, whole, _refused) = seen.join().expect("the reader never saw a torn line");
    assert!(looks > 0 && whole > 0, "the reader never saw the layers");
    let _ = h.finish();
}
