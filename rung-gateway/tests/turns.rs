//! S8: the turns door over real HTTP: the record folded into turns on the
//! host's side. Records are written by hand (no host runs), so every number
//! asked for is known; a "running" instance is one whose state lock a test holds.

use std::path::PathBuf;

use rung_gateway::Settings;
use rung_host::statelock::StateLock;
use rung_testkit::TempDir;
use serde_json::{Value, json};

const READ_TOKEN: &str = "view-token-canary-s8-3310";
/// A sentinel with a known key shape: found by the redactor with no help.
const SHAPED: &str = "sk-SENTINELs8shaped0123456789abcdef";
const T0: i64 = 1_791_208_186_000;

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

fn line(seq: u64, kind: &str, body: Value) -> Value {
    let mut m = body.as_object().cloned().unwrap_or_default();
    m.insert("seq".into(), seq.into());
    m.insert("at".into(), (T0 + seq as i64 * 1000).into());
    m.insert("kind".into(), kind.into());
    Value::Object(m)
}

/// Lines numbered from 1.
fn numbered(kinds: Vec<(&str, Value)>) -> Vec<Value> {
    kinds
        .into_iter()
        .enumerate()
        .map(|(i, (k, b))| line(i as u64 + 1, k, b))
        .collect()
}

struct Fleet {
    root: TempDir,
    locks: Vec<StateLock>,
}

impl Fleet {
    fn new(tag: &str) -> Self {
        let root = TempDir::new(tag);
        std::fs::create_dir_all(root.path().join("registry")).unwrap();
        Fleet {
            root,
            locks: Vec::new(),
        }
    }

    fn state(&self, id: &str) -> PathBuf {
        self.root.path().join(id).join("state")
    }

    fn add(&self, id: &str, lines: &[Value]) {
        let state = self.state(id);
        std::fs::create_dir_all(state.join("record")).unwrap();
        let entry = json!({
            "schema": 1, "id": id, "name": id, "state_dir": state, "config": null,
            "workspace": self.root.path().join(id).join("workspace"),
            "address": null, "pid": 4242, "started": T0, "version": "0.3.2",
        });
        std::fs::write(
            self.root.path().join("registry").join(format!("{id}.json")),
            serde_json::to_string_pretty(&entry).unwrap(),
        )
        .unwrap();
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(state.join("record/seg-00000001.ndjson"), text).unwrap();
    }

    fn hold(&mut self, id: &str) {
        self.locks
            .push(StateLock::acquire(&self.state(id)).expect("lock"));
    }

    async fn serve(&self) -> Gw {
        let running = rung_gateway::start(Settings {
            listen: "127.0.0.1:0".parse().unwrap(),
            app_dir: None,
            allowed_hosts: vec![],
            read_only_tokens: vec![READ_TOKEN.into()],
            registry_dir: Some(self.root.path().join("registry")),
            instances: vec![],
        })
        .await
        .unwrap();
        Gw {
            base: format!("http://{}", running.addr),
            _running: running,
        }
    }
}

struct Gw {
    base: String,
    _running: rung_gateway::Running,
}

struct Answer {
    status: u16,
    body: String,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("not JSON ({e}): {}", self.body))
    }
}

impl Gw {
    async fn get(&self, path: &str) -> Answer {
        self.send(client().get(format!("{}{path}", self.base)))
            .await
    }

    async fn get_as_reader(&self, path: &str) -> Answer {
        self.send(
            client()
                .get(format!("{}{path}", self.base))
                .bearer_auth(READ_TOKEN),
        )
        .await
    }

    async fn send(&self, rb: reqwest::RequestBuilder) -> Answer {
        let r = rb.send().await.unwrap();
        Answer {
            status: r.status().as_u16(),
            body: r.text().await.unwrap(),
        }
    }
}

/// A turn that did a tool call, replied, and ended.
fn turn_log(
    turn: u64,
    tool: &str,
    id: &str,
    input: Value,
    result: &str,
    is_error: bool,
    reply: &str,
) -> Value {
    json!({"turn": turn, "header": "h", "recall": false, "messages": [
        {"role": "assistant", "content": [
            {"type": "text", "text": "  thinking aloud  "},
            {"type": "tool_use", "id": id, "name": tool, "input": input}]},
        {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": id, "content": result, "is_error": is_error}]},
        {"role": "assistant", "content": reply}]})
}

/// Six turns and the lines around them:
/// 1 completed (tool call with result, reply, usage); 2 bounded, an owner steer
/// arrived during it; 3 failed, took that item, one tool refused; 4 never ended
/// (cut off: turn 5 starts); 5 ended with only a `final_text`, a tool call line
/// and no turn.log; 6 never ended and is the newest. Turn 9 has only a decision
/// line: it never started, so it is no turn.
fn story() -> Vec<Value> {
    numbered(vec![
        ("host.start", json!({"pid": 1})),
        ("boundary", json!({"n": 1})),
        (
            "turn.started",
            json!({"turn": 1, "turn_kind": "free", "mode": "free", "model": "m-a", "epoch": 1, "project": null}),
        ),
        (
            "tool.call",
            json!({"turn": 1, "name": "read", "group": "core", "ok": true}),
        ),
        (
            "llm.call",
            json!({"turn": 1, "call": 1, "prompt_tokens": 100, "cached_tokens": 40, "completion_tokens": 10, "cost_usd": 0.01}),
        ),
        (
            "llm.call",
            json!({"turn": 1, "call": 2, "prompt_tokens": 150, "cached_tokens": 100, "completion_tokens": 20, "cost_usd": 0.02}),
        ),
        (
            "turn.log",
            turn_log(
                1,
                "read",
                "c-1",
                json!({"path": "a.md"}),
                "the file text",
                false,
                "all done",
            ),
        ),
        (
            "turn.ended",
            json!({"turn": 1, "status": "completed", "elapsed_ms": 930, "turn_kind": "free", "model": "m-a2", "final_text": "all done"}),
        ),
        ("boundary", json!({"n": 2})),
        (
            "turn.started",
            json!({"turn": 2, "turn_kind": "committed", "mode": "committed:p", "model": "m-a", "epoch": 1, "project": "notes"}),
        ),
        (
            "stimulus.accepted",
            json!({"item": {"id": "m-1", "kind": "peer", "role": "owner", "channel": "owner", "at": T0 + 11_500, "intent": "steer", "text": "also the appendix"}}),
        ),
        (
            "turn.ended",
            json!({"turn": 2, "status": "bounded", "elapsed_ms": 5, "final_text": "ran out of room"}),
        ),
        ("boundary", json!({"n": 3})),
        (
            "stimulus.admitted",
            json!({"turn": 3, "ids": ["m-1", "m-missing"]}),
        ),
        (
            "turn.started",
            json!({"turn": 3, "turn_kind": "free", "mode": "free", "model": "m-a", "epoch": 1}),
        ),
        (
            "tool.refused",
            json!({"turn": 3, "name": "write", "group": "workspace_write", "why": "not_enabled", "message": "that tool group is off"}),
        ),
        (
            "turn.ended",
            json!({"turn": 3, "status": "error", "failure": {"class": "provider", "message": "the model failed"}, "elapsed_ms": 70}),
        ),
        ("decision.admit", json!({"turn": 9, "choice": {}})),
        (
            "turn.started",
            json!({"turn": 4, "turn_kind": "free", "mode": "free", "model": "m-a", "epoch": 1}),
        ),
        (
            "turn.started",
            json!({"turn": 5, "turn_kind": "free", "mode": "free", "model": "m-a", "epoch": 1}),
        ),
        (
            "tool.call",
            json!({"turn": 5, "name": "trace", "group": "core", "ok": true}),
        ),
        (
            "turn.ended",
            json!({"turn": 5, "status": "completed", "final_text": "reply of five"}),
        ),
        (
            "turn.started",
            json!({"turn": 6, "turn_kind": "free", "mode": "free", "model": "m-a", "epoch": 1}),
        ),
    ])
}

#[tokio::test]
async fn a_record_folds_to_exactly_the_documented_turns() {
    let mut f = Fleet::new("s8-fold");
    f.add("alpha", &story());
    f.hold("alpha"); // a host is alive
    let gw = f.serve().await;

    let a = gw.get("/api/i/alpha/v1/turns").await;
    assert_eq!(a.status, 200, "{}", a.body);
    let p = a.json();
    assert_eq!(
        (p["total"].as_u64(), p["order"].as_str()),
        (Some(6), Some("desc"))
    );
    assert!(p["damaged"].is_null() && p["next"].is_null());
    assert_eq!(p["redaction"]["env_unset"], json!([]));
    let order: Vec<u64> = p["turns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["turn"].as_u64().unwrap())
        .collect();
    assert_eq!(
        order,
        [6, 5, 4, 3, 2, 1],
        "newest first; turn 9 never started and is no turn"
    );

    let by = |n: u64| {
        p["turns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["turn"] == n)
            .unwrap()
            .clone()
    };
    // status
    let statuses: Vec<String> = (1..=6)
        .map(|n| by(n)["status"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        statuses,
        [
            "completed",
            "bounded",
            "failed",
            "interrupted",
            "completed",
            "running"
        ]
    );

    // turn 1: facts, reply from the log then final_text already inside it, usage sums
    let t1 = by(1);
    assert_eq!(
        (t1["kind"].as_str(), t1["model"].as_str()),
        (Some("free"), Some("m-a2")),
        "turn.ended's model wins"
    );
    assert_eq!(t1["text"], "thinking aloud\n\nall done");
    assert_eq!(
        t1["llm"],
        json!({"calls": 2, "prompt_tokens": 250, "cached_tokens": 140, "completion_tokens": 30, "cost_usd": 0.03})
    );
    assert_eq!(
        (
            t1["tool_calls"].as_u64(),
            t1["asked"].as_u64(),
            t1["elapsed_ms"].as_u64()
        ),
        (Some(1), Some(0), Some(930))
    );
    assert_eq!(
        (t1["first_seq"].as_u64(), t1["last_seq"].as_u64()),
        (Some(3), Some(8))
    );
    assert_eq!(t1["started_at"].as_i64(), Some(T0 + 3000));
    assert_eq!(t1["ended_at"].as_i64(), Some(T0 + 8000));

    // turn 2: no turn.log, so the reply is the final text alone; the project is read
    assert_eq!(by(2)["text"], "ran out of room");
    assert_eq!(by(2)["project"], "notes");
    // turn 3: failed, with the record's failure object whole, one refused call, one asked item
    let t3 = by(3);
    assert_eq!(
        t3["failure"],
        json!({"class": "provider", "message": "the model failed"})
    );
    assert_eq!(
        (t3["tool_calls"].as_u64(), t3["asked"].as_u64()),
        (Some(1), Some(1))
    );
    // turn 5: no turn.log, so its call is the tool.call line; the reply is its final text
    assert_eq!(
        (by(5)["text"].as_str(), by(5)["tool_calls"].as_u64()),
        (Some("reply of five"), Some(1))
    );
    // turn 4 and 6 never ended: cut off, and running (the lock is held, newest)
    assert!(by(4)["ended_at"].is_null() && by(6)["ended_at"].is_null());
}

#[tokio::test]
async fn one_turn_is_whole_with_its_asked_items_and_calls() {
    let mut f = Fleet::new("s8-one");
    f.add("alpha", &story());
    f.hold("alpha");
    let gw = f.serve().await;

    // turn 1: the call from the log, with its input and result whole
    let t1 = gw.get("/api/i/alpha/v1/turns/1").await.json();
    assert_eq!(
        t1["calls"],
        json!([{"name": "read", "group": null, "ok": true, "refused": null,
                "input": {"path": "a.md"}, "result": "the file text"}])
    );
    assert_eq!(t1["asked_items"], json!([]));
    assert_eq!(t1["tool_calls"], 1);
    assert!(t1["damaged"].is_null());

    // turn 3: the owner's steer, sent during turn 2, taken at turn 3's boundary; the
    // refused call; an id the record never accepted is skipped
    let t3 = gw.get("/api/i/alpha/v1/turns/3").await.json();
    assert_eq!(
        t3["asked_items"],
        json!([{"id": "m-1", "from": "owner", "role": "owner", "intent": "steer",
                "text": "also the appendix", "sent_at": T0 + 11_500,
                "seen_at": T0 + 14_000, "during": 2}])
    );
    assert_eq!(
        t3["calls"],
        json!([{"name": "write", "group": "workspace_write", "ok": false,
                "refused": "not_enabled", "input": null, "result": "that tool group is off"}])
    );
    // a turn with no log: its call is the tool.call line, input and result unknown
    let t5 = gw.get("/api/i/alpha/v1/turns/5").await.json();
    assert_eq!(
        t5["calls"],
        json!([{"name": "trace", "group": "core", "ok": true, "refused": null, "input": null, "result": null}])
    );

    // refusals: no such turn (also one that only a decision line mentions), a bad number, parameters
    for n in ["7", "9", "9999"] {
        let a = gw.get(&format!("/api/i/alpha/v1/turns/{n}")).await;
        assert_eq!(a.status, 404, "{n}");
        assert_eq!(a.json()["error"], "no_such_turn");
    }
    for bad in [
        "/api/i/alpha/v1/turns/x",
        "/api/i/alpha/v1/turns/-1",
        "/api/i/alpha/v1/turns/1?limit=2",
    ] {
        assert_eq!(gw.get(bad).await.status, 400, "{bad}");
    }
}

#[tokio::test]
async fn pages_by_turn_number_miss_and_repeat_nothing() {
    let f = {
        let f = Fleet::new("s8-pages");
        f.add("alpha", &story());
        f
    };
    let gw = f.serve().await;
    let nums = |a: &Answer| -> Vec<u64> {
        a.json()["turns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["turn"].as_u64().unwrap())
            .collect()
    };

    // newest first, two at a time, following `next` as `before`
    let mut seen = Vec::new();
    let mut path = "/api/i/alpha/v1/turns?limit=2".to_string();
    loop {
        let a = gw.get(&path).await;
        assert_eq!(a.status, 200, "{}", a.body);
        seen.extend(nums(&a));
        match a.json()["next"].as_u64() {
            Some(n) => path = format!("/api/i/alpha/v1/turns?limit=2&before={n}"),
            None => break,
        }
    }
    assert_eq!(seen, [6, 5, 4, 3, 2, 1]);

    // oldest first from a cursor, following `next` as `after`
    let mut seen = Vec::new();
    let mut path = "/api/i/alpha/v1/turns?limit=4&after=0".to_string();
    loop {
        let a = gw.get(&path).await;
        assert_eq!(a.json()["order"], "asc");
        seen.extend(nums(&a));
        match a.json()["next"].as_u64() {
            Some(n) => path = format!("/api/i/alpha/v1/turns?limit=4&after={n}"),
            None => break,
        }
    }
    assert_eq!(seen, [1, 2, 3, 4, 5, 6]);

    // no maximum on the page, an empty page past the end, total always the whole
    let all = gw
        .get("/api/i/alpha/v1/turns?limit=18446744073709551615")
        .await;
    assert_eq!(nums(&all).len(), 6);
    let none = gw.get("/api/i/alpha/v1/turns?before=1").await;
    assert_eq!(
        (nums(&none).len(), none.json()["total"].as_u64()),
        (0, Some(6))
    );
    assert!(none.json()["next"].is_null());

    // 400s
    for bad in [
        "limit=0",
        "limit=x",
        "limit=1&limit=2",
        "before=1&after=2",
        "offset=1",
        "order=asc",
        "before=",
        "after=-1",
    ] {
        let a = gw.get(&format!("/api/i/alpha/v1/turns?{bad}")).await;
        assert_eq!(a.status, 400, "{bad}: {}", a.body);
        assert_eq!(a.json()["error"], "bad_request");
    }
    assert_eq!(gw.get("/api/i/nobody/v1/turns").await.status, 404);
}

#[tokio::test]
async fn a_huge_reply_and_result_come_back_whole() {
    let big = "é".repeat(2_000_000);
    let lines = numbered(vec![
        ("host.start", json!({})),
        (
            "turn.started",
            json!({"turn": 1, "turn_kind": "free", "mode": "free", "model": "m", "epoch": 1}),
        ),
        (
            "turn.log",
            turn_log(1, "read", "c-1", json!({"path": "big"}), &big, false, &big),
        ),
        ("turn.ended", json!({"turn": 1, "status": "completed"})),
    ]);
    let f = Fleet::new("s8-huge");
    f.add("alpha", &lines);
    let gw = f.serve().await;
    let one = gw.get("/api/i/alpha/v1/turns/1").await.json();
    assert_eq!(
        one["calls"][0]["result"].as_str().unwrap().chars().count(),
        2_000_000
    );
    let want = format!("thinking aloud\n\n{big}");
    assert_eq!(one["text"].as_str().unwrap(), want, "the reply is not cut");
    let list = gw.get("/api/i/alpha/v1/turns").await.json();
    assert_eq!(list["turns"][0]["text"].as_str().unwrap(), want);
}

#[tokio::test]
async fn a_planted_secret_is_in_the_record_and_in_no_byte_of_any_answer() {
    let lines = numbered(vec![
        ("host.start", json!({})),
        (
            "stimulus.accepted",
            json!({"item": {"id": "m-1", "kind": "peer", "role": "owner", "channel": "owner", "at": T0, "text": format!("the key is {SHAPED}")}}),
        ),
        ("stimulus.admitted", json!({"turn": 1, "ids": ["m-1"]})),
        (
            "turn.started",
            json!({"turn": 1, "turn_kind": "free", "mode": "free", "model": "m", "epoch": 1}),
        ),
        (
            "turn.log",
            turn_log(
                1,
                "write",
                "c-1",
                json!({"content": format!("token={SHAPED}")}),
                &format!("wrote {SHAPED}"),
                false,
                &format!("I kept {SHAPED} safe"),
            ),
        ),
        (
            "turn.ended",
            json!({"turn": 1, "status": "completed", "final_text": format!("done with {SHAPED}")}),
        ),
    ]);
    let f = Fleet::new("s8-secret");
    f.add("alpha", &lines);
    let gw = f.serve().await;
    let on_disk =
        std::fs::read_to_string(f.state("alpha").join("record/seg-00000001.ndjson")).unwrap();
    assert!(on_disk.contains(SHAPED), "the record stays verbatim");
    for path in ["/api/i/alpha/v1/turns", "/api/i/alpha/v1/turns/1"] {
        let a = gw.get(path).await;
        assert_eq!(a.status, 200);
        assert!(!a.body.contains(SHAPED), "{path}: {}", a.body);
        assert!(a.body.contains("[redacted]"), "{path}");
    }
    // the asked item and the tool input are in the detail, redacted
    let one = gw.get("/api/i/alpha/v1/turns/1").await.json();
    assert!(
        one["asked_items"][0]["text"]
            .as_str()
            .unwrap()
            .contains("[redacted]")
    );
    assert!(
        one["calls"][0]["input"]["content"]
            .as_str()
            .unwrap()
            .contains("[redacted]")
    );
}

#[tokio::test]
async fn both_roles_read_a_stopped_instance_and_nobody_writes() {
    let f = Fleet::new("s8-roles");
    f.add("alpha", &story()); // no lock held: a stopped instance
    let gw = f.serve().await;
    for path in ["/api/i/alpha/v1/turns", "/api/i/alpha/v1/turns/2"] {
        assert_eq!(gw.get(path).await.status, 200, "the owner reads {path}");
        assert_eq!(
            gw.get_as_reader(path).await.status,
            200,
            "the reader reads {path}"
        );
    }
    // a stopped instance has no running turn: 6 is cut off, not running
    let p = gw.get("/api/i/alpha/v1/turns").await.json();
    assert_eq!(p["turns"][0]["status"], "interrupted");
    // a wrong token is 401; a write is refused
    let bad = gw
        .send(
            client()
                .get(format!("{}/api/i/alpha/v1/turns", gw.base))
                .bearer_auth("nobody"),
        )
        .await;
    assert_eq!(bad.status, 401);
    let w = gw
        .send(client().post(format!("{}/api/i/alpha/v1/turns", gw.base)))
        .await;
    assert_eq!(w.status, 405);
    let w = gw
        .send(
            client()
                .post(format!("{}/api/i/alpha/v1/turns", gw.base))
                .bearer_auth(READ_TOKEN),
        )
        .await;
    assert_eq!(w.status, 403);
}

#[tokio::test]
async fn a_damaged_record_is_read_as_far_as_it_is_intact() {
    let mut lines = story();
    let f = Fleet::new("s8-damaged");
    f.add("alpha", &lines);
    // append a line that is not the next number: everything before it is intact
    let seg = f.state("alpha").join("record/seg-00000001.ndjson");
    let mut text = std::fs::read_to_string(&seg).unwrap();
    lines.push(line(99, "turn.started", json!({"turn": 7})));
    text.push_str(&format!("{}\n", lines.last().unwrap()));
    std::fs::write(&seg, text).unwrap();
    let gw = f.serve().await;
    let p = gw.get("/api/i/alpha/v1/turns").await.json();
    assert_eq!(p["total"], 6, "turn 7 is past the damage");
    assert_eq!(p["damaged"]["after_seq"], 23);
    assert_eq!(gw.get("/api/i/alpha/v1/turns/7").await.status, 404);
    assert_eq!(gw.get("/api/i/alpha/v1/turns/6").await.status, 200);
}
