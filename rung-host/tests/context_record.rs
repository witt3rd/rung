//! S4: the request record (layers, whole, redacted) and the context door.
//! A sentinel secret is never put in an assertion message.

mod common;

use std::sync::{Arc, Mutex};

use common::ctx::*;
use rung_host::canon;
use rung_host::clock::{Clock, HOUR, SimClock};
use rung_host::engine::{EngineTurn, TurnEngine, TurnRequest};
use rung_host::inbox::Item;
use rung_host::memory::MemoryHost;
use rung_host::pack::request_bytes;
use rung_host::record::Line;
use rung_host::redact::Redactor;
use rung_host::sim::{self, MockEngine, SIM_START};
use rung_std::llm::{ChatMessage, MessageContentBlock};
use serde_json::{Value, json};

/// The mock, remembering the canonical bytes of every request it was handed
/// (the first call of each turn: the thread as the host assembled it).
struct Spy {
    inner: Arc<MockEngine>,
    seen: Mutex<Vec<(u64, Vec<u8>)>>,
}

impl TurnEngine for Spy {
    fn name(&self) -> &str {
        "mock"
    }
    fn turn(&self, req: TurnRequest) -> EngineTurn {
        let bytes = request_bytes(
            &req.tools.definitions(),
            &req.thread.system_prompt,
            &req.thread.messages,
        );
        self.seen.lock().unwrap().push((req.turn, bytes));
        self.inner.turn(req)
    }
}

fn spied(sc: &mut sim::Scenario) -> Arc<Spy> {
    let clock: Arc<dyn Clock> = Arc::new(SimClock::new(SIM_START));
    sc.clock = Some(clock.clone());
    let mock = Arc::new(MockEngine::new(
        sc.mock.clone(),
        clock,
        sim::FaultInjector::new(Vec::new()),
    ));
    let spy = Arc::new(Spy {
        inner: mock,
        seen: Mutex::new(Vec::new()),
    });
    sc.engine = Some(spy.clone());
    spy
}


// ─── the tests ───────────────────────────────────────────────────────────────

/// The layers are written whole where they are made, once: a 50-call turn
/// writes one `pack.layers`, not 50; the hashes join them to the calls.
#[test]
fn the_layers_are_recorded_whole_and_once() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("ctx-layers");
    let (sc, script) = scripted(guard.path(), 50, 3);
    let out = sim::run(sc);
    let seen = script.seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    let layers: Vec<&Line> = out.lines.iter().filter(|l| l.kind == "pack.layers").collect();
    assert_eq!(layers.len(), 1, "one pack.layers for the whole run");
    let l = layers[0];
    assert_eq!(l.get("tools"), &serde_json::to_value(&seen[0].tools).unwrap());
    assert_eq!(l.str("system"), seen[0].system);
    let calls: Vec<&Line> = out.lines.iter().filter(|l| l.kind == "llm.call").collect();
    assert_eq!(calls.len(), 150, "three turns of 50 calls");
    for c in &calls {
        assert_eq!(&c.get("prefix")["s_hash"], l.get("s_hash"));
    }
    // the epoch layer: whole, with the hash the calls carry
    let rolls: Vec<&Line> = out
        .lines
        .iter()
        .filter(|l| l.kind == "epoch.rollover")
        .collect();
    assert_eq!(rolls.len(), 1);
    assert_eq!(
        rolls[0].str("slow"),
        seen[0].messages[0].content.as_text().unwrap()
    );
    assert_eq!(rolls[0].get("l_hash"), &calls[0].get("prefix")["l_hash"]);
    // the header as sent, and the recall as its own field
    let logs: Vec<&Line> = out.lines.iter().filter(|l| l.kind == "turn.log").collect();
    assert_eq!(logs.len(), 3);
    for (i, t) in logs.iter().enumerate() {
        let sent = seen[i].messages.last().unwrap().content.as_text().unwrap();
        assert_eq!(t.str("header_sent"), sent);
        assert_eq!(t.str("header_sent"), t.str("header"), "no recall this turn");
        assert!(t.get("recall_text").is_null());
    }
}

/// A swap of the stable layer records its layers, once, with the new hash.
#[test]
fn a_swap_writes_the_new_layers_once() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("ctx-swap");
    let (mut sc, script) = scripted(guard.path(), 3, 4);
    // the swap happens at the third boundary, before its turn is built
    struct Swap {
        host: Arc<std::sync::OnceLock<std::sync::Weak<rung_host::presence::Host>>>,
        polls: u32,
    }
    impl rung_host::inbox::Source for Swap {
        fn poll(&mut self, _now: i64, _seen: &std::collections::BTreeSet<String>) -> Vec<Item> {
            self.polls += 1;
            if self.polls == 3
                && let Some(h) = self.host.get().and_then(std::sync::Weak::upgrade)
            {
                let mut tools = rung_host::toolbox::superset(false);
                tools.truncate(3);
                h.swap_stable(tools, "a different system text".into());
            }
            Vec::new()
        }
    }
    let cell = Arc::new(std::sync::OnceLock::new());
    sc.sources.push(Box::new(Swap {
        host: cell.clone(),
        polls: 0,
    }));
    let (host, rec, _) = sim::build(sc);
    let _ = cell.set(Arc::downgrade(&host));
    let _ = host.run(rec);
    let lines = host.record_lines().unwrap();
    let layers: Vec<&Line> = lines.iter().filter(|l| l.kind == "pack.layers").collect();
    assert_eq!(layers.len(), 2, "the start and the swap, one each");
    assert_eq!(layers[1].str("system"), "a different system text");
    assert_eq!(layers[1].get("tools").as_array().unwrap().len(), 3);
    assert_ne!(layers[0].get("s_hash"), layers[1].get("s_hash"));
    assert_eq!(layers[1].str("cause"), "swap");
    assert!(lines.iter().any(|l| l.kind == "pack.swap"));
    // the calls after the swap carry the new hash
    let last = lines.iter().rfind(|l| l.kind == "llm.call").unwrap();
    assert_eq!(&last.get("prefix")["s_hash"], layers[1].get("s_hash"));
    drop(script);
}

/// The door gives the bands in the order the model sees them, and what it
/// gives rebuilds the request the engine was handed, byte for byte.
#[test]
fn the_door_rebuilds_the_request_that_was_sent() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("ctx-door");
    let (sc, script) = scripted(guard.path(), 6, 3);
    let _ = sim::run(sc);
    let seen = script.seen.lock().unwrap();
    let red = Redactor::new();
    for (turn, call) in [(1u64, 1usize), (2, 1), (2, 4), (3, 6)] {
        let v = door(guard.path(), turn, Some(call as u64), &red).unwrap();
        assert_eq!(v["turn"], turn);
        assert_eq!(v["call"], call);
        assert_eq!(v["calls"], 6);
        assert_eq!(v["recorded"], true);
        assert_eq!(
            names(&v),
            ["tools", "system", "epoch", "history", "recall", "message", "this_turn"]
                .iter()
                .filter(|n| **n != "recall")
                .map(|n| n.to_string())
                .collect::<Vec<_>>(),
            "no recall in this run"
        );
        let s = &seen[(turn - 1) as usize];
        // rebuild: tools, system, then each message the model saw
        let defs: Vec<Value> = band(&v, "tools")["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| serde_json::from_str(i["text"].as_str().unwrap()).unwrap())
            .collect();
        let mut msgs: Vec<Value> = vec![json!({"content": band(&v, "epoch")["text"], "role": "user"})];
        for row in band(&v, "history")["rows"].as_array().unwrap() {
            msgs.push(json!({"content": row["header"], "role": "user"}));
            msgs.extend(row["messages"].as_array().unwrap().iter().cloned());
        }
        msgs.push(json!({"content": band(&v, "message")["text"], "role": "user"}));
        msgs.extend(
            band(&v, "this_turn")["messages"]
                .as_array()
                .unwrap()
                .iter()
                .cloned(),
        );
        let mut got = canon::bytes(&Value::Array(defs));
        got.push(b'\n');
        got.extend(canon::of(band(&v, "system")["text"].as_str().unwrap()));
        for m in &msgs {
            got.push(b'\n');
            got.extend(canon::bytes(m));
        }
        let want = request_bytes(&s.tools, &s.system, &wire(s, 6, call));
        assert!(got == want, "turn {turn} call {call}: the bands rebuild the request");
    }
}

/// Out of range is a plain refusal; so is a turn that is not in the record.
#[test]
fn a_call_or_turn_that_is_not_there_is_refused() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("ctx-range");
    let (sc, _script) = scripted(guard.path(), 4, 2);
    let _ = sim::run(sc);
    let red = Redactor::new();
    assert_eq!(door(guard.path(), 1, Some(5), &red).unwrap_err().0, 404);
    assert_eq!(door(guard.path(), 1, Some(0), &red).unwrap_err().0, 404);
    assert_eq!(door(guard.path(), 9, Some(1), &red).unwrap_err().0, 404);
    assert_eq!(door(guard.path(), 1, Some(5), &red).unwrap_err().1, "no_such_call");
    assert_eq!(door(guard.path(), 9, None, &red).unwrap_err().1, "no_such_turn");
    // without `call`, the first call
    assert_eq!(door(guard.path(), 1, None, &red).unwrap()["call"], 1);
    assert_eq!(door(guard.path(), 2, Some(4), &red).unwrap()["call"], 4);
}

/// Cache markers follow the provider's cached-token count in wire order, and
/// the provider's own counts ride along.
#[test]
fn the_bands_carry_the_provider_counts_and_cache_marks() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("ctx-cache");
    let (sc, script) = scripted(guard.path(), 4, 2);
    let _ = sim::run(sc);
    let seen = script.seen.lock().unwrap();
    let red = Redactor::new();
    let v = door(guard.path(), 2, Some(3), &red).unwrap();
    // call 3 of the script: prompt 1100, cached 1050
    assert_eq!(v["provider"]["prompt_tokens"], 1100);
    assert_eq!(v["provider"]["cached_tokens"], 1050);
    assert_eq!(v["provider"]["cache_write_tokens"], 0);
    assert!(v["provider"]["expected_cached_tokens"].is_number());
    // the host's own estimates, by its rule: a token for four bytes
    let s = &seen[1];
    let tools_bytes = canon::of(&s.tools).len() + 1;
    assert_eq!(band(&v, "tools")["est_tokens"], canon::tokens(tools_bytes));
    assert_eq!(
        band(&v, "system")["est_tokens"],
        canon::tokens(canon::of(&s.system).len())
    );
    // marks never go back from new to cached, and the first bands are cached
    let marks: Vec<&str> = v["bands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["cache"].as_str().unwrap())
        .collect();
    let rank = |m: &str| match m {
        "cached" => 0,
        "partly" => 1,
        _ => 2,
    };
    assert!(marks.windows(2).all(|w| rank(w[0]) <= rank(w[1])), "{marks:?}");
    assert_eq!(marks[0], "cached");
    assert_eq!(*marks.last().unwrap(), "new");
    // the two breakpoints: the end of the stable layer, the end of the epoch layer
    let b = v["boundaries"].as_array().unwrap();
    assert_eq!(b.len(), 2);
    assert_eq!(b[0]["after"], "system");
    assert_eq!(b[1]["after"], "epoch");
    assert!(v["estimate_tokens"].as_u64().unwrap() > 0);
}


/// A whole tool-definition set, a 100,000-character epoch layer and a
/// 200,000-character recall go through the record and the door whole.
#[test]
fn big_layers_round_trip_whole() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("ctx-big");
    // a first run, then the agent's note (what the next epoch's slow layer carries)
    let (sc, _) = scripted(guard.path(), 2, 1);
    drop(sim::run(sc));
    let note = format!("{}END-OF-NOTE", "n".repeat(100_000));
    append_line(guard.path(), "note.written", json!({"turn": 1, "text": note}));
    // a restart: its first epoch carries the note; the owner asks, and memory recalls
    let (mut sc, script) = scripted(guard.path(), 3, 3);
    let clk: Arc<dyn Clock> = Arc::new(SimClock::new(SIM_START + HOUR));
    sc.clock = Some(clk.clone());
    let script2 = Script::new(3, clk);
    sc.engine = Some(script2.clone());
    drop(script);
    sc.sources.push(owner_says(SIM_START + HOUR, "what is the deploy window"));
    let recall = long_recall(2_100, None);
    assert!(recall.len() > 200_000);
    sc.memory_host = Some(Arc::new(MemoryHost::new(
        Arc::new(Big {
            text: recall.clone(),
        }),
        "s",
    )));
    let out = sim::run(sc);
    let seen = script2.seen.lock().unwrap();
    let with_recall = out
        .lines
        .iter()
        .find(|l| l.kind == "turn.log" && l.get("recall") == &json!(true))
        .expect("a turn with a recall");
    let turn = with_recall.u64("turn");
    let s = seen.iter().find(|s| s.turn == turn).unwrap();
    let red = Redactor::new();
    let v = door(guard.path(), turn, Some(2), &red).unwrap();
    // the epoch layer, whole
    let epoch = band(&v, "epoch")["text"].as_str().unwrap();
    assert!(epoch.len() > 100_000 && epoch.contains("END-OF-NOTE"));
    assert_eq!(epoch, s.messages[0].content.as_text().unwrap());
    // the recall, whole, and the message it follows: together the header as sent
    let rtext = band(&v, "recall")["text"].as_str().unwrap();
    let mtext = band(&v, "message")["text"].as_str().unwrap();
    assert!(rtext.len() > 200_000 && rtext.contains("rl02099"));
    assert_eq!(
        format!("{mtext}\n{rtext}"),
        s.messages.last().unwrap().content.as_text().unwrap()
    );
    // the tool definitions, whole, each its own row
    let items = band(&v, "tools")["items"].as_array().unwrap();
    assert_eq!(items.len(), s.tools.len());
    assert!(items.len() > 5);
    for (i, t) in s.tools.iter().enumerate() {
        assert_eq!(items[i]["name"], t.name);
        assert_eq!(
            items[i]["text"].as_str().unwrap().as_bytes(),
            canon::of(t).as_slice()
        );
    }
    assert_eq!(band(&v, "system")["text"], s.system);
    // and the line on disk holds it all in one piece
    assert!(out
        .lines
        .iter()
        .any(|l| l.kind == "epoch.rollover" && l.str("slow") == epoch));
    assert_eq!(with_recall.str("recall_text"), rtext);
    assert_eq!(
        with_recall.str("header_sent").len(),
        mtext.len() + 1 + rtext.len()
    );
}

const SENT: &str = "plainsentinels4ctx-0123456789";
const SHAPED: &str = "sk-s4ctxshapedsentinel0123456789abcdef";

/// A secret in the system text or in the recall never reaches the record
/// file or the door: it reads `[redacted]` where it was.
#[test]
fn a_planted_secret_reaches_neither_the_record_nor_the_door() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("ctx-secret");
    let (mut sc, script) = scripted(guard.path(), 3, 2);
    sc.config.pinned = vec![format!("the vault code is {SENT}"), format!("the key is {SHAPED}")];
    sc.redactor = Some(Redactor::new().with_secret(SENT));
    sc.sources.push(owner_says(SIM_START, "what is the code"));
    sc.memory_host = Some(Arc::new(MemoryHost::new(
        Arc::new(Big {
            text: long_recall(50, Some((7, SENT))) + "\n" + &format!("also {SHAPED}"),
        }),
        "s",
    )));
    let out = sim::run(sc);
    let seen = script.seen.lock().unwrap();
    // the model was still shown what it was shown
    assert!(seen[0].system.contains(SENT), "the host's own request is untouched");
    assert!(seen[0].messages.last().unwrap().content.as_text().unwrap().contains(SENT));
    // the file
    let bytes = record_bytes(guard.path());
    assert!(!contains(&bytes, SENT), "the record holds the plain secret");
    assert!(!contains(&bytes, SHAPED), "the record holds the key");
    assert!(contains(&bytes, "[redacted]"));
    // the door, with a redactor that knows neither
    let with_recall = out
        .lines
        .iter()
        .find(|l| l.kind == "turn.log" && l.get("recall") == &json!(true))
        .expect("a recall");
    let turn = with_recall.u64("turn");
    for red in [Redactor::new(), Redactor::new().with_secret(SENT)] {
        let v = door(guard.path(), turn, Some(1), &red).unwrap();
        let all = v.to_string();
        assert!(!all.contains(SENT), "the door shows the plain secret");
        assert!(!all.contains(SHAPED), "the door shows the key");
        assert!(band(&v, "system")["text"].as_str().unwrap().contains("[redacted]"));
        assert!(band(&v, "recall")["text"].as_str().unwrap().contains("[redacted]"));
        let tools = band(&v, "tools").to_string();
        assert!(tools.contains("input_schema"));
    }
}

/// A record from before the layers were recorded still answers: with what it
/// has, and a plain word that the rest predates context recording.
#[test]
fn an_old_record_answers_what_it_has() {
    sim::test_timeout(60);
    let guard = sim::temp_dir_guard("ctx-old");
    std::fs::create_dir_all(guard.path().join("record")).unwrap();
    std::fs::write(guard.path().join("record/seg-00000001.ndjson"), "").unwrap();
    let mut n = 0u64;
    let mut put = |kind: &str, body: Value| {
        n += 1;
        let mut m = body.as_object().cloned().unwrap();
        m.insert("seq".into(), n.into());
        m.insert("at".into(), (SIM_START + n as i64).into());
        m.insert("kind".into(), kind.into());
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(guard.path().join("record/seg-00000001.ndjson"))
            .unwrap();
        writeln!(f, "{}", canon::string(&Value::Object(m))).unwrap();
    };
    put("host.start", json!({"pid": 1, "config": {}}));
    put("epoch.rollover", json!({"from": 0, "to": 1, "cause": "start", "l1": "epoch 1", "first_turn": 1}));
    let call = |turn: u64, c: u64, prompt: u64, cached: u64| {
        json!({"turn": turn, "call": c, "epoch": 1, "rung": 0, "model_requested": "m", "model_served": "m",
               "provider": "p", "prompt_tokens": prompt, "cached_tokens": cached, "cache_write_tokens": 0,
               "completion_tokens": 5, "prefix": {"s_hash": "aaaa", "l_hash": "bbbb", "log_len_bytes": 10,
               "expected_cached_tokens": 100}})
    };
    let msgs = |t: u64| {
        json!([serde_json::to_value(ChatMessage::assistant_with_blocks(vec![
            MessageContentBlock::ToolUse { id: format!("t{t}"), name: "look".into(), input: json!({}), cache: None }
        ])).unwrap(),
        serde_json::to_value(ChatMessage::user_with_blocks(vec![
            MessageContentBlock::ToolResult { tool_use_id: format!("t{t}"), content: "found".into(), images: vec![], is_error: false, cache: None }
        ])).unwrap(),
        serde_json::to_value(ChatMessage::assistant(format!("done {t}"))).unwrap()])
    };
    for t in 1..=2u64 {
        put("turn.started", json!({"turn": t, "epoch": 1, "turn_kind": "free", "model": "m"}));
        put("llm.call", call(t, 1, 900, 100));
        put("llm.call", call(t, 2, 1000, 900));
        put("turn.log", json!({"turn": t, "header": format!("[turn {t}]"), "recall": false, "messages": msgs(t)}));
        put("turn.ended", json!({"turn": t, "status": "completed"}));
    }
    let red = Redactor::new();
    let v = door(guard.path(), 2, Some(2), &red).unwrap();
    assert_eq!(v["recorded"], false);
    assert!(v["note"].as_str().unwrap().contains("predates context recording"));
    assert_eq!(names(&v), ["history", "message", "this_turn"]);
    assert_eq!(band(&v, "history")["rows"].as_array().unwrap().len(), 1);
    assert_eq!(band(&v, "message")["text"], "[turn 2]");
    assert_eq!(band(&v, "this_turn")["messages"].as_array().unwrap().len(), 2);
    assert_eq!(v["provider"]["prompt_tokens"], 1000);
    assert_eq!(v["provider"]["cached_tokens"], 900);
    let missing: Vec<&str> = v["missing"].as_array().unwrap().iter().map(|m| m.as_str().unwrap()).collect();
    assert_eq!(missing, ["tools", "system", "epoch", "recall"]);
    // and a call that is not there is still a refusal, not an error of the format
    assert_eq!(door(guard.path(), 2, Some(3), &red).unwrap_err().0, 404);
}

/// One digest per turn of a seeded busy run.
fn digests(tag: &str, budget: Option<usize>, turns: u64) -> (Vec<String>, usize) {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard(tag);
    let mut sc = sim::Scenario::new(guard.path(), 11);
    sc.world = common::busy_world(11, 3 * HOUR);
    sc.max_turns = Some(turns);
    if let Some(b) = budget {
        sc.config.epoch_budget_tokens = b;
    }
    let spy = spied(&mut sc);
    let out = sim::run(sc);
    let rolls = out
        .lines
        .iter()
        .filter(|l| l.kind == "epoch.rollover")
        .count();
    let seen = spy.seen.lock().unwrap();
    // the scratch directory's name (it carries the process id) is not part
    // of what is being pinned
    let dir = guard.path().to_string_lossy().to_string();
    let d = seen
        .iter()
        .map(|(t, b)| {
            let text = String::from_utf8_lossy(b).replace(&dir, "<DIR>");
            format!("{t}:{}", rung_host::canon::hash(text.as_bytes()))
        })
        .collect();
    (d, rolls)
}

/// The request bytes the provider is handed do not change: one digest per
/// turn of a seeded busy run, folded to one, pinned from the host before the
/// request record existed (a run with no rollover after the first, and one
/// with several).
#[test]
fn the_request_bytes_are_unchanged() {
    let (plain, _) = digests("ctx-golden", None, 14);
    let (rolled, rolls) = digests("ctx-golden-roll", Some(6_000), 14);
    assert!(rolls >= 3, "the second run rolls over");
    let fold = |d: &[String]| rung_host::canon::hash(d.join(",").as_bytes());
    assert_eq!(fold(&plain), "87e3a16912b958ae686c2699db0c2e96");
    assert_eq!(fold(&rolled), "1a0a338bfe6d60ed480273722384e4cb");
}
