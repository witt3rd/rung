//! S3: reorder and cancel on the host's own `POST /v1/queue/{id}/move` and
//! `DELETE /v1/queue/{id}` doors, over real HTTP against the `rung-host` binary
//! (`sim --acp-http`, real clock, mock engine). Written first against a host
//! that had neither door.
//!
//! Three hosts hold items still, with no sleep on a clock:
//! * `quota`: the model's day quota is spent, so the host waits and every item
//!   it is given waits (owner items from the door, peers from a file the host
//!   takes from its inbox directory);
//! * `wedge`: the first turn reads a task from the inbox and never ends, so
//!   that task is being answered for as long as the test runs, and every item
//!   added later waits;
//! * `plain`: an ordinary host that answers what it is given quickly.
//!
//! A test asserts only about the items it added, by id: a running host has
//! items of its own besides. No assertion message prints a sentinel.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rung_host::record::{Line, Record};
use rung_host::sim;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");
const OWNER_TOKEN: &str = "s3-owner-token-canary-6612";
const PEER_TOKEN: &str = "s3-peer-token-canary-8835";
const SHAPED: &str = "sk-SENTINELs3shaped0123456789abcdef";

struct Server {
    child: Child,
    base: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Quota,
    Wedge,
    Plain,
}

fn spawn(kind: Kind, state: &Path, inbox: &Path, extra: &[&str]) -> Server {
    std::fs::create_dir_all(inbox).unwrap();
    let mut args: Vec<&str> = vec![
        "sim",
        "--clock",
        "real",
        "--call-ms",
        "20,40",
        "--no-commit",
        "--acp-http",
        "127.0.0.1:0",
        "--acp-token-env",
        "owner=S3_OWNER_TOKEN",
        "--acp-token-env",
        "peer=S3_PEER_TOKEN",
        "--state",
        state.to_str().unwrap(),
        "--inbox",
        inbox.to_str().unwrap(),
    ];
    match kind {
        Kind::Quota => args.extend(["--quota", "1,60"]),
        Kind::Wedge => args.extend(["--wedge-at", "1"]),
        Kind::Plain => {}
    }
    let mut child = Command::new(BIN)
        .args(&args)
        .args(extra)
        .env("S3_OWNER_TOKEN", OWNER_TOKEN)
        .env("S3_PEER_TOKEN", PEER_TOKEN)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rung-host");
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut addr = String::new();
    for _ in 0..50 {
        let mut line = String::new();
        if stderr.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Some(url) = line.split(" ACP HTTP at ").nth(1) {
            addr = url
                .trim()
                .strip_prefix("http://")
                .and_then(|s| s.strip_suffix("/acp"))
                .unwrap_or("")
                .to_string();
            break;
        }
    }
    assert!(!addr.is_empty(), "no listen line");
    std::thread::spawn(move || {
        let mut sink = String::new();
        while stderr.read_line(&mut sink).unwrap_or(0) > 0 {
            sink.clear();
        }
    });
    Server {
        child,
        base: format!("http://{addr}"),
    }
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
}

struct Answer {
    status: u16,
    body: String,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("not JSON ({e}): {}", self.body))
    }

    fn message(&self) -> String {
        self.json()["message"].as_str().unwrap_or("").to_string()
    }
}

fn answer(rb: reqwest::blocking::RequestBuilder) -> Answer {
    let r = rb.send().unwrap();
    Answer {
        status: r.status().as_u16(),
        body: r.text().unwrap(),
    }
}

fn get(s: &Server, path: &str) -> Answer {
    answer(
        client()
            .get(format!("{}{path}", s.base))
            .bearer_auth(OWNER_TOKEN),
    )
}

fn add(s: &Server, key: &str, text: &str) -> Answer {
    answer(
        client()
            .post(format!("{}/v1/queue", s.base))
            .bearer_auth(OWNER_TOKEN)
            .header("content-type", "application/json")
            .body(json!({"text": text, "key": key}).to_string()),
    )
}

fn move_with(s: &Server, token: Option<&str>, id: &str, body: &Value) -> Answer {
    let rb = client()
        .post(format!("{}/v1/queue/{id}/move", s.base))
        .header("content-type", "application/json")
        .body(body.to_string());
    answer(match token {
        Some(t) => rb.bearer_auth(t),
        None => rb,
    })
}

/// Move `id` in front of `before` (the end of its group when none).
fn mv(s: &Server, id: &str, before: Option<&str>) -> Answer {
    move_with(s, Some(OWNER_TOKEN), id, &json!({ "before": before }))
}

fn mv_key(s: &Server, id: &str, before: Option<&str>, key: &str) -> Answer {
    move_with(
        s,
        Some(OWNER_TOKEN),
        id,
        &json!({"before": before, "key": key}),
    )
}

fn cancel_with(s: &Server, token: Option<&str>, id: &str) -> Answer {
    let rb = client().delete(format!("{}/v1/queue/{id}", s.base));
    answer(match token {
        Some(t) => rb.bearer_auth(t),
        None => rb,
    })
}

fn cancel(s: &Server, id: &str) -> Answer {
    cancel_with(s, Some(OWNER_TOKEN), id)
}

fn lines(state: &Path) -> Vec<Line> {
    Record::read_dir(state.join("record")).unwrap_or_default()
}

fn of_kind(state: &Path, kind: &str) -> Vec<Line> {
    lines(state)
        .into_iter()
        .filter(|l| l.kind == kind)
        .collect()
}

/// How many lines of the record are the owner's doing: an item taken from the
/// owner or a peer, a move, a withdrawal. A refused request must leave this where
/// it was. (A running host writes lines of its own: an acknowledgement to the
/// owner, expectations settling, so the record's whole length says nothing.)
fn writes(state: &Path) -> usize {
    lines(state)
        .iter()
        .filter(|l| match l.kind.as_str() {
            "queue.moved" => true,
            "stimulus.disposed" => l.str("disposition") == "withdrawn",
            "stimulus.accepted" => l.get("item")["role"] != "host",
            _ => false,
        })
        .count()
}

/// Poll `f` until it holds, for at most `secs`: a bound on a wait for the host
/// to do something it does at once, not a pause for timing.
fn until(secs: u64, what: &str, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("gave up waiting for {what}");
}

/// The ids among `of` that wait, in the list's order.
fn waiting(s: &Server, of: &[&str]) -> Vec<String> {
    let v = get(s, "/v1/queue?state=waiting&limit=100000").json();
    v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_string())
        .filter(|id| of.contains(&id.as_str()))
        .collect()
}

fn item(s: &Server, id: &str) -> Value {
    let v = get(s, "/v1/queue?state=all&limit=100000").json();
    v["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == id)
        .unwrap_or_else(|| panic!("no item {id}"))
        .clone()
}

/// A host whose model quota is spent: it is waiting, and holds what it is given.
fn quota(state: &Path, inbox: &Path) -> Server {
    let s = spawn(Kind::Quota, state, inbox, &[]);
    until(20, "the host to run out of quota", || {
        !of_kind(state, "degraded").is_empty()
    });
    s
}

fn peer(s: &Server, inbox: &Path, name: &str, text: &str) {
    std::fs::write(
        inbox.join(format!("{name}.msg")),
        json!({"role": "peer", "text": text}).to_string(),
    )
    .unwrap();
    until(20, "the host to take the peer's message", || {
        item_exists(s, name)
    });
}

fn item_exists(s: &Server, id: &str) -> bool {
    let v = get(s, "/v1/queue?state=all&limit=100000").json();
    v["items"].as_array().unwrap().iter().any(|i| i["id"] == id)
}

/// A host that is answering the inbox's `blocker` task and will not stop.
fn wedged(state: &Path, inbox: &Path) -> Server {
    std::fs::create_dir_all(inbox).unwrap();
    std::fs::write(
        inbox.join("blocker.msg"),
        json!({"role": "owner", "text": "a task that does not end"}).to_string(),
    )
    .unwrap();
    let s = spawn(Kind::Wedge, state, inbox, &[]);
    until(20, "the blocker to be answered", || {
        item_exists(&s, "blocker") && item(&s, "blocker")["state"] == "answering"
    });
    s
}

// ─── reorder ─────────────────────────────────────────────────────────────────

#[test]
fn an_owner_item_moves_up_and_down_and_the_line_is_in_the_record_before_the_answer() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-owner");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    for k in ["o1", "o2", "o3"] {
        assert_eq!(add(&s, k, &format!("task {k}")).status, 201);
    }
    let all = ["o1", "o2", "o3"];
    assert_eq!(waiting(&s, &all), ["o1", "o2", "o3"]);
    let held_before = item(&s, "o2")["held"].clone();
    // up: o3 to the front
    let a = mv(&s, "o3", Some("o1"));
    assert_eq!(a.status, 200, "{}", a.body);
    let v = a.json();
    assert_eq!(v["id"], "o3");
    assert_eq!(v["moved"], true);
    assert_eq!(v["repeat"], false);
    assert_eq!(v["item"]["id"], "o3");
    // the answer is the proof: the line is whole and on disk by now
    let moved = of_kind(&state, "queue.moved");
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].str("id"), "o3");
    assert_eq!(moved[0].get("before"), "o1");
    assert_eq!(waiting(&s, &all), ["o3", "o1", "o2"]);
    assert_eq!(item(&s, "o3")["position"], v["item"]["position"]);
    // down: o3 in front of o2, then to the end
    assert_eq!(mv(&s, "o3", Some("o2")).status, 200);
    assert_eq!(waiting(&s, &all), ["o1", "o3", "o2"]);
    assert_eq!(mv(&s, "o3", None).status, 200);
    assert_eq!(waiting(&s, &all), ["o1", "o2", "o3"]);
    let last = of_kind(&state, "queue.moved").pop().unwrap();
    assert_eq!(last.str("id"), "o3");
    assert!(last.get("before").is_null());
    // a move changes order and nothing else: the desk's words are the same
    assert_eq!(item(&s, "o2")["held"], held_before);
}

#[test]
fn a_move_to_its_own_place_is_a_no_op_that_writes_nothing() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-noop");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    for k in ["n1", "n2", "n3"] {
        assert_eq!(add(&s, k, "x").status, 201);
    }
    let before = writes(&state);
    // n1 is already in front of n2; n3 is already last; n2 in front of itself
    for (id, b) in [("n1", Some("n2")), ("n3", None), ("n2", Some("n2"))] {
        let a = mv(&s, id, b);
        assert_eq!(a.status, 200, "{}", a.body);
        assert_eq!(a.json()["moved"], false);
        assert_eq!(a.json()["repeat"], false);
    }
    assert_eq!(writes(&state), before, "no line was written");
    assert_eq!(waiting(&s, &["n1", "n2", "n3"]), ["n1", "n2", "n3"]);
}

#[test]
fn a_peer_item_never_passes_an_owner_item_in_either_direction() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-group");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    assert_eq!(add(&s, "g-o1", "owner one").status, 201);
    assert_eq!(add(&s, "g-o2", "owner two").status, 201);
    peer(&s, &inbox, "g-atlas", "from atlas");
    peer(&s, &inbox, "g-bolt", "from bolt");
    let all = ["g-o1", "g-o2", "g-atlas", "g-bolt"];
    // owner items on top, then peers
    assert_eq!(waiting(&s, &all), all);
    let w = writes(&state);
    let n = writes(&state);
    // a peer above an owner item: refused in words
    for owner in ["g-o1", "g-o2"] {
        let a = mv(&s, "g-atlas", Some(owner));
        assert_eq!(a.status, 409, "{}", a.body);
        assert_eq!(a.json()["error"], "group");
        let m = a.message();
        assert!(m.contains("owner"), "{m}");
    }
    // an owner item below a peer: refused in words
    let a = mv(&s, "g-o1", Some("g-atlas"));
    assert_eq!(a.status, 409, "{}", a.body);
    assert_eq!(a.json()["error"], "group");
    assert!(a.message().contains("owner"), "{}", a.message());
    let a = mv(&s, "g-o2", Some("g-bolt"));
    assert_eq!(a.status, 409);
    assert_eq!(writes(&state), w);
    assert_eq!(writes(&state), n, "a refusal writes no line");
    assert_eq!(waiting(&s, &all), all);
    // within a group a move is fine, and the groups stay as they were
    assert_eq!(mv(&s, "g-bolt", Some("g-atlas")).status, 200);
    assert_eq!(mv(&s, "g-o2", Some("g-o1")).status, 200);
    assert_eq!(
        waiting(&s, &all),
        ["g-o2", "g-o1", "g-bolt", "g-atlas"],
        "owner items first, peers after"
    );
    // the end of a group is the end of that group, never past another's
    assert_eq!(mv(&s, "g-o2", None).status, 200);
    assert_eq!(waiting(&s, &all), ["g-o1", "g-o2", "g-bolt", "g-atlas"]);
}

#[test]
fn an_id_in_the_path_may_be_percent_encoded() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-encoded");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    assert_eq!(add(&s, "e-o1", "owner").status, 201);
    peer(&s, &inbox, "e one", "a file with a space in its name");
    peer(&s, &inbox, "e-two", "another");
    assert_eq!(mv(&s, "e%20one", None).status, 200);
    assert_eq!(
        waiting(&s, &["e one", "e-two"]),
        ["e-two", "e one"].map(String::from)
    );
    let a = cancel(&s, "e%20one");
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(item(&s, "e one")["state"], "withdrawn");
}

#[test]
fn a_move_of_an_item_that_is_not_waiting_is_refused_in_words_and_writes_nothing() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-refused");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = wedged(&state, &inbox);
    assert_eq!(add(&s, "w1", "one").status, 201);
    assert_eq!(add(&s, "w2", "two").status, 201);
    assert_eq!(add(&s, "w3", "three").status, 201);
    assert_eq!(cancel(&s, "w3").status, 200);
    let n = writes(&state);
    // the item being answered right now cannot be moved, nor put in front of
    let a = mv(&s, "blocker", Some("w1"));
    assert_eq!(a.status, 409, "{}", a.body);
    assert_eq!(a.json()["error"], "not_waiting");
    assert!(a.message().contains("answer"), "{}", a.message());
    let a = mv(&s, "w1", Some("blocker"));
    assert_eq!(a.status, 409, "{}", a.body);
    assert_eq!(a.json()["error"], "not_waiting");
    // a withdrawn one cannot be moved either, nor an unknown one
    let a = mv(&s, "w3", Some("w1"));
    assert_eq!(a.status, 409, "{}", a.body);
    assert!(a.message().contains("withdrawn"), "{}", a.message());
    let a = mv(&s, "w1", Some("w3"));
    assert_eq!(a.status, 409, "{}", a.body);
    let a = mv(&s, "no-such-item", Some("w1"));
    assert_eq!(a.status, 404, "{}", a.body);
    assert_eq!(a.json()["error"], "not_found");
    let a = mv(&s, "w1", Some("no-such-item"));
    assert_eq!(a.status, 404, "{}", a.body);
    assert_eq!(writes(&state), n, "no line was written by any refusal");
    assert_eq!(waiting(&s, &["w1", "w2"]), ["w1", "w2"]);
}

#[test]
fn an_answered_item_cannot_be_moved_or_cancelled() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-answered");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = spawn(Kind::Plain, &state, &inbox, &[]);
    assert_eq!(add(&s, "a1", "one").status, 201);
    assert_eq!(add(&s, "a2", "two").status, 201);
    until(30, "both items to be answered", || {
        item(&s, "a1")["state"] == "answered" && item(&s, "a2")["state"] == "answered"
    });
    let w = writes(&state);
    let a = mv(&s, "a1", Some("a2"));
    assert_eq!(a.status, 409, "{}", a.body);
    assert_eq!(a.json()["error"], "not_waiting");
    let c = cancel(&s, "a1");
    assert_eq!(c.status, 409, "{}", c.body);
    assert_eq!(c.json()["error"], "not_waiting");
    assert!(c.message().contains("answered"), "{}", c.message());
    // the host writes lines of its own as it runs: only ours are counted
    assert_eq!(writes(&state), w);
}

// ─── cancel ──────────────────────────────────────────────────────────────────

#[test]
fn a_cancelled_item_is_withdrawn_once_and_never_reaches_the_agent() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-cancel");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    {
        let s = quota(&state, &inbox);
        for k in ["c1", "c2", "c3"] {
            assert_eq!(add(&s, k, &format!("task {k}")).status, 201);
        }
        let a = cancel(&s, "c2");
        assert_eq!(a.status, 200, "{}", a.body);
        let v = a.json();
        assert_eq!(v["id"], "c2");
        assert_eq!(v["withdrawn"], true);
        assert_eq!(v["repeat"], false);
        assert_eq!(v["item"]["state"], "withdrawn");
        assert_eq!(v["item"]["disposition"], "withdrawn");
        // the line is on disk by the time the answer is read: the one path
        // the ACP cancel uses, one disposition `withdrawn`
        let d: Vec<Line> = of_kind(&state, "stimulus.disposed")
            .into_iter()
            .filter(|l| l.str("id") == "c2")
            .collect();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].str("disposition"), "withdrawn");
        assert_eq!(waiting(&s, &["c1", "c2", "c3"]), ["c1", "c3"]);
        // a repeat is a no-op, said so: no second line
        let again = cancel(&s, "c2");
        assert_eq!(again.status, 200, "{}", again.body);
        assert_eq!(again.json()["repeat"], true);
        assert_eq!(again.json()["withdrawn"], true);
        let n = of_kind(&state, "stimulus.disposed")
            .into_iter()
            .filter(|l| l.str("id") == "c2")
            .count();
        assert_eq!(n, 1);
        // an unknown id
        assert_eq!(cancel(&s, "never-was").status, 404);
    }
    // the host comes back with a model: it reads c1 and c3, never c2
    let s = spawn(Kind::Plain, &state, &inbox, &[]);
    until(30, "c1 and c3 to be answered", || {
        item(&s, "c1")["state"] == "answered" && item(&s, "c3")["state"] == "answered"
    });
    for l in of_kind(&state, "stimulus.admitted") {
        let ids = l.get("ids").to_string() + &l.get("digests").to_string();
        assert!(!ids.contains("c2"), "the cancelled item was admitted");
    }
    assert_eq!(item(&s, "c2")["state"], "withdrawn");
    let turns: Vec<Line> = of_kind(&state, "turn.log");
    for t in turns {
        assert!(
            !t.get("messages").to_string().contains("task c2"),
            "the cancelled item's text reached a turn"
        );
    }
}

#[test]
fn the_item_being_answered_cannot_be_cancelled() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-cancel-answering");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = wedged(&state, &inbox);
    let w = writes(&state);
    let n = writes(&state);
    let a = cancel(&s, "blocker");
    assert_eq!(a.status, 409, "{}", a.body);
    assert_eq!(a.json()["error"], "not_waiting");
    let m = a.message();
    assert!(m.contains("answer") && m.contains("turn"), "{m}");
    assert_eq!(writes(&state), w);
    assert_eq!(writes(&state), n);
    assert_eq!(item(&s, "blocker")["state"], "answering");
}

// ─── idempotence ─────────────────────────────────────────────────────────────

#[test]
fn a_repeat_with_the_same_key_is_one_line_and_a_reused_key_is_refused() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-key");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    for k in ["k1", "k2", "k3"] {
        assert_eq!(add(&s, k, "x").status, 201);
    }
    let first = mv_key(&s, "k3", Some("k1"), "mv-1");
    assert_eq!(first.status, 200, "{}", first.body);
    assert_eq!(first.json()["moved"], true);
    // a second move happens, then the first is retried: it is one line still,
    // and it does not undo the second
    assert_eq!(mv(&s, "k1", None).status, 200);
    let lines_before = of_kind(&state, "queue.moved").len();
    let again = mv_key(&s, "k3", Some("k1"), "mv-1");
    assert_eq!(again.status, 200, "{}", again.body);
    assert_eq!(again.json()["repeat"], true);
    assert_eq!(again.json()["moved"], false);
    assert_eq!(of_kind(&state, "queue.moved").len(), lines_before);
    assert_eq!(waiting(&s, &["k1", "k2", "k3"]), ["k3", "k2", "k1"]);
    // the same key for a different move: refused, nothing written
    let other = mv_key(&s, "k2", Some("k3"), "mv-1");
    assert_eq!(other.status, 409, "{}", other.body);
    assert_eq!(other.json()["error"], "key_reused");
    assert_eq!(of_kind(&state, "queue.moved").len(), lines_before);
    // the key survives a restart: it is in the line
    let key_lines: Vec<Line> = of_kind(&state, "queue.moved")
        .into_iter()
        .filter(|l| l.get("key") == "mv-1")
        .collect();
    assert_eq!(key_lines.len(), 1);
}

#[test]
fn twenty_concurrent_equal_moves_make_one_line() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-same");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    for k in ["x1", "x2", "x3"] {
        assert_eq!(add(&s, k, "x").status, 201);
    }
    let base = s.base.clone();
    let go = std::sync::Barrier::new(20);
    let statuses: Vec<u16> = std::thread::scope(|t| {
        let hs: Vec<_> = (0..20)
            .map(|_| {
                t.spawn(|| {
                    go.wait();
                    answer(
                        client()
                            .post(format!("{base}/v1/queue/x3/move"))
                            .header("content-type", "application/json")
                            .body(json!({"before": "x1"}).to_string())
                            .bearer_auth(OWNER_TOKEN),
                    )
                    .status
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(statuses.iter().all(|s| *s == 200));
    assert_eq!(of_kind(&state, "queue.moved").len(), 1);
    assert_eq!(waiting(&s, &["x1", "x2", "x3"]), ["x3", "x1", "x2"]);
}

#[test]
fn twenty_concurrent_opposing_moves_end_as_the_later_line_says_and_every_line_is_whole() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-race");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    for k in ["r1", "r2"] {
        assert_eq!(add(&s, k, "x").status, 201);
    }
    let base = s.base.clone();
    let go = std::sync::Barrier::new(20);
    let statuses: Vec<u16> = std::thread::scope(|t| {
        let hs: Vec<_> = (0..20)
            .map(|n| {
                let (go, base) = (&go, &base);
                t.spawn(move || {
                    // ten ask for r1 in front of r2, ten for r2 in front of r1
                    let (id, before) = if n % 2 == 0 {
                        ("r1", "r2")
                    } else {
                        ("r2", "r1")
                    };
                    go.wait();
                    answer(
                        client()
                            .post(format!("{base}/v1/queue/{id}/move"))
                            .header("content-type", "application/json")
                            .body(json!({"before": before}).to_string())
                            .bearer_auth(OWNER_TOKEN),
                    )
                    .status
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(statuses.iter().all(|s| *s == 200));
    // the record reads whole, in order, with no gap
    let all = lines(&state);
    for (i, l) in all.iter().enumerate() {
        assert_eq!(l.seq, i as u64 + 1, "a gap or a torn line in the record");
    }
    let moved = of_kind(&state, "queue.moved");
    assert!(!moved.is_empty() && moved.len() <= 20);
    for m in &moved {
        assert!(m.str("id") == "r1" || m.str("id") == "r2");
        assert!(m.get("before").is_string());
    }
    // later line wins: the order is the last line's
    let last = moved.last().unwrap();
    let want: Vec<String> = if last.str("id") == "r1" {
        vec!["r1".into(), "r2".into()]
    } else {
        vec!["r2".into(), "r1".into()]
    };
    assert_eq!(waiting(&s, &["r1", "r2"]), want);
    // and the same after the record is read afresh (a restart's fold)
    drop(s);
    let s = quota(&state, &inbox);
    assert_eq!(waiting(&s, &["r1", "r2"]), want);
}

// ─── restart ─────────────────────────────────────────────────────────────────

#[test]
fn the_order_survives_a_restart_and_is_the_order_the_agent_reads() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-restart");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let ids = ["z1", "z2", "z3", "z4"];
    {
        let s = quota(&state, &inbox);
        for k in ids {
            assert_eq!(add(&s, k, &format!("text of {k}")).status, 201);
        }
        assert_eq!(mv(&s, "z4", Some("z1")).status, 200);
        assert_eq!(mv(&s, "z2", None).status, 200);
        assert_eq!(waiting(&s, &ids), ["z4", "z1", "z3", "z2"]);
        // killed with no goodbye
    }
    {
        let s = quota(&state, &inbox);
        assert_eq!(waiting(&s, &ids), ["z4", "z1", "z3", "z2"]);
    }
    // the host comes back with a model: its first turn reads them in that order
    let s = spawn(Kind::Plain, &state, &inbox, &[]);
    until(30, "all four to be answered", || {
        ids.iter().all(|k| item(&s, k)["state"] == "answered")
    });
    let read: Vec<String> = of_kind(&state, "stimulus.admitted")
        .iter()
        .flat_map(|l| {
            l.get("ids")
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
        })
        .filter(|id| ids.contains(&id.as_str()))
        .collect();
    assert_eq!(read, ["z4", "z1", "z3", "z2"]);
}

// ─── who may, and when ───────────────────────────────────────────────────────

#[test]
fn only_the_owner_token_may_move_or_cancel() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-roles");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    for k in ["t1", "t2"] {
        assert_eq!(add(&s, k, "x").status, 201);
    }
    let n = writes(&state);
    let body = json!({"before": "t1"});
    assert_eq!(move_with(&s, None, "t2", &body).status, 401);
    assert_eq!(move_with(&s, Some("not-a-token"), "t2", &body).status, 401);
    let p = move_with(&s, Some(PEER_TOKEN), "t2", &body);
    assert_eq!(p.status, 403, "{}", p.body);
    assert_eq!(cancel_with(&s, None, "t2").status, 401);
    assert_eq!(cancel_with(&s, Some("not-a-token"), "t2").status, 401);
    let p = cancel_with(&s, Some(PEER_TOKEN), "t2");
    assert_eq!(p.status, 403, "{}", p.body);
    assert_eq!(writes(&state), n, "nothing was written");
    assert_eq!(waiting(&s, &["t1", "t2"]), ["t1", "t2"]);
    // other methods are not these doors'
    let g = get(&s, "/v1/queue/t1");
    assert_eq!(g.status, 405, "{}", g.body);
    let g = get(&s, "/v1/queue/t1/move");
    assert_eq!(g.status, 405, "{}", g.body);
    let d = answer(
        client()
            .delete(format!("{}/v1/queue/t1/move", s.base))
            .bearer_auth(OWNER_TOKEN),
    );
    assert_eq!(d.status, 405, "{}", d.body);
    let p = answer(
        client()
            .post(format!("{}/v1/queue/t1", s.base))
            .header("content-type", "application/json")
            .body("{}")
            .bearer_auth(OWNER_TOKEN),
    );
    assert_eq!(p.status, 405, "{}", p.body);
    let d = answer(
        client()
            .delete(format!("{}/v1/queue", s.base))
            .bearer_auth(OWNER_TOKEN),
    );
    assert_eq!(d.status, 405, "{}", d.body);
    // a path that is not an item's
    let p = answer(
        client()
            .delete(format!("{}/v1/queue/a/b/c", s.base))
            .bearer_auth(OWNER_TOKEN),
    );
    assert_eq!(p.status, 404, "{}", p.body);
    assert_eq!(writes(&state), n);
}

#[test]
fn a_bad_move_is_refused_in_words_and_writes_nothing() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-bad");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    for k in ["b1", "b2"] {
        assert_eq!(add(&s, k, "x").status, 201);
    }
    let n = writes(&state);
    let cases: Vec<(Value, &str)> = vec![
        (json!({}), "before"),
        (json!({"before": 7}), "before"),
        (json!({"before": "b1", "colour": "red"}), "colour"),
        (json!({"before": "b1", "key": ""}), "key"),
        (json!({"before": "b1", "key": "has space"}), "key"),
        (json!({"before": "b1", "key": 5}), "key"),
        (json!([1, 2]), "object"),
    ];
    for (body, word) in cases {
        let a = move_with(&s, Some(OWNER_TOKEN), "b2", &body);
        assert_eq!(a.status, 400, "{body}: {}", a.body);
        let m = a.message();
        assert!(m.contains(word), "{body}: {m}");
    }
    let r = answer(
        client()
            .post(format!("{}/v1/queue/b2/move", s.base))
            .header("content-type", "application/json")
            .body("{not json")
            .bearer_auth(OWNER_TOKEN),
    );
    assert_eq!(r.status, 400);
    let r = answer(
        client()
            .post(format!("{}/v1/queue/b2/move", s.base))
            .header("content-type", "text/plain")
            .body("hello")
            .bearer_auth(OWNER_TOKEN),
    );
    assert_eq!(r.status, 415);
    assert_eq!(writes(&state), n);
    // `before: null` is the end of the group, said outright
    assert_eq!(mv(&s, "b1", None).status, 200);
}

#[test]
fn a_host_that_is_stopping_refuses_a_move_and_a_cancel_with_start_it_first() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-stopping");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let stop_file = guard.path().join("STOP");
    let s = spawn(
        Kind::Plain,
        &state,
        &inbox,
        &["--stop-file", stop_file.to_str().unwrap()],
    );
    assert_eq!(add(&s, "p1", "x").status, 201);
    std::fs::write(&stop_file, "stop").unwrap();
    let mut seen = (None, None);
    let end = Instant::now() + Duration::from_secs(30);
    while Instant::now() < end {
        let m = client()
            .post(format!("{}/v1/queue/p1/move", s.base))
            .header("content-type", "application/json")
            .body(json!({"before": null}).to_string())
            .bearer_auth(OWNER_TOKEN)
            .send();
        let c = client()
            .delete(format!("{}/v1/queue/p1", s.base))
            .bearer_auth(OWNER_TOKEN)
            .send();
        let (Ok(m), Ok(c)) = (m, c) else { break };
        let (ms, cs) = (m.status().as_u16(), c.status().as_u16());
        if ms == 409 && cs == 409 {
            seen = (Some(m.text().unwrap()), Some(c.text().unwrap()));
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    for body in [seen.0, seen.1] {
        let v: Value = serde_json::from_str(&body.expect("a 409 while stopping")).unwrap();
        assert_eq!(v["error"], "not_running");
        assert!(v["message"].as_str().unwrap().contains("start it first"));
    }
}

// ─── content ─────────────────────────────────────────────────────────────────

#[test]
fn a_200000_character_item_survives_a_move_and_a_cancel_whole() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-big");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    let text: String = (0..200_000)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    assert_eq!(add(&s, "big-1", "small first").status, 201);
    assert_eq!(add(&s, "big-2", &text).status, 201);
    let a = mv(&s, "big-2", Some("big-1"));
    assert_eq!(a.status, 200, "{}", a.body);
    assert!(
        a.json()["item"]["text"].as_str().unwrap() == text,
        "the move's answer holds the whole text"
    );
    assert_eq!(waiting(&s, &["big-1", "big-2"]), ["big-2", "big-1"]);
    let listed = item(&s, "big-2");
    assert!(listed["text"].as_str().unwrap() == text, "listed whole");
    let c = cancel(&s, "big-2");
    assert_eq!(c.status, 200);
    assert!(c.json()["item"]["text"].as_str().unwrap() == text);
    let listed = item(&s, "big-2");
    assert_eq!(listed["state"], "withdrawn");
    assert!(listed["text"].as_str().unwrap() == text, "listed whole");
    // the record's line is the original, byte for byte
    let acc: Vec<Line> = of_kind(&state, "stimulus.accepted")
        .into_iter()
        .filter(|l| l.get("item")["id"] == "big-2")
        .collect();
    assert_eq!(acc.len(), 1);
    assert!(acc[0].get("item")["text"].as_str().unwrap() == text);
}

#[test]
fn every_error_is_redacted_even_one_that_names_the_id_it_was_given() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s3-move-redact");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    assert_eq!(add(&s, "d1", "x").status, 201);
    let mut wire = String::new();
    for a in [
        cancel(&s, SHAPED),
        cancel(&s, OWNER_TOKEN),
        mv(&s, SHAPED, Some("d1")),
        mv(&s, "d1", Some(SHAPED)),
        mv(&s, "d1", Some(OWNER_TOKEN)),
    ] {
        assert_eq!(a.status, 404, "{}", a.body);
        wire.push_str(&a.body);
    }
    assert!(!wire.contains(SHAPED), "a shaped secret reached the wire");
    assert!(!wire.contains(OWNER_TOKEN), "a token reached the wire");
    assert!(wire.contains("[redacted]"));
}

// ─── S2 and S3 together: one order, in the list and in the turn header ───────

/// An owner item added with an `intent`.
fn add_intent(s: &Server, key: &str, intent: &str) -> Answer {
    answer(
        client()
            .post(format!("{}/v1/queue", s.base))
            .bearer_auth(OWNER_TOKEN)
            .header("content-type", "application/json")
            .body(
                json!({"text": format!("text of {key}"), "key": key, "intent": intent}).to_string(),
            ),
    )
}

/// The ids among `of` in the order the agent's turn headers list them: each `turn.log`'s
/// header, row by row (`  [channel/id @time ...] text`), turns in order.
fn read_in_headers(state: &Path, of: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for l in of_kind(state, "turn.log") {
        for row in l.get("header").as_str().unwrap_or("").lines() {
            let Some(rest) = row.strip_prefix("  [") else {
                continue;
            };
            let Some((_, tail)) = rest.split_once('/') else {
                continue;
            };
            let id = tail.split_whitespace().next().unwrap_or("");
            if of.contains(&id) {
                out.push(id.to_string());
            }
        }
    }
    out
}

/// What the owner moved is what the agent reads: the order of the rows in the turn header is
/// the order the list showed, not the order the items were accepted in, and the move
/// survives a restart.
#[test]
fn the_turn_header_lists_items_in_the_order_the_owner_moved_them_to() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s23-header-moves");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let ids = ["h1", "h2", "h3", "h4"];
    let shown;
    {
        let s = quota(&state, &inbox);
        for k in ids {
            assert_eq!(add(&s, k, &format!("text of {k}")).status, 201);
        }
        assert_eq!(mv(&s, "h4", Some("h1")).status, 200);
        assert_eq!(mv(&s, "h2", None).status, 200);
        shown = waiting(&s, &ids);
        assert_eq!(shown, ["h4", "h1", "h3", "h2"]);
        // killed with no goodbye
    }
    let s = spawn(Kind::Plain, &state, &inbox, &[]);
    until(30, "all four to be answered", || {
        ids.iter().all(|k| item(&s, k)["state"] == "answered")
    });
    assert_eq!(
        read_in_headers(&state, &ids),
        shown,
        "the header follows the list"
    );
}

/// An owner's `free` item is its own group: a move across `free` and `next` is refused in
/// words with nothing written (it used to answer `moved: true` and change nothing), while a
/// `next` and a `steer` are one group and move past each other.
#[test]
fn a_move_between_an_owner_free_item_and_a_next_item_is_refused_and_writes_nothing() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s23-free-group");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let s = quota(&state, &inbox);
    let ids = ["g-n1", "g-s1", "g-f1", "g-f2", "g-n2"];
    for (k, intent) in [
        ("g-n1", "next"),
        ("g-s1", "steer"),
        ("g-f1", "free"),
        ("g-f2", "free"),
        ("g-n2", "next"),
    ] {
        assert_eq!(add_intent(&s, k, intent).status, 201);
    }
    // read order: next and steer as sent, then free as sent
    assert_eq!(waiting(&s, &ids), ["g-n1", "g-s1", "g-n2", "g-f1", "g-f2"]);
    let n = writes(&state);
    for (id, before) in [
        ("g-f1", Some("g-n1")),
        ("g-f2", Some("g-s1")),
        ("g-n2", Some("g-f1")),
        ("g-n1", Some("g-f2")),
        ("g-s1", Some("g-f1")),
    ] {
        let a = mv(&s, id, before);
        assert_eq!(a.status, 409, "{id} before {before:?}: {}", a.body);
        assert_eq!(a.json()["error"], "group");
        let m = a.json()["message"].as_str().unwrap().to_string();
        assert!(m.contains("nothing was written"), "{m}");
        assert!(m.contains("free"), "the refusal says why: {m}");
    }
    // the end of a group stays in the group: a `next` sent to the end is below the `next`
    // items and above the `free` ones
    let a = mv(&s, "g-n1", None);
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.json()["moved"], true);
    assert_eq!(writes(&state), n + 1, "only the legal move wrote a line");
    assert_eq!(waiting(&s, &ids), ["g-s1", "g-n2", "g-n1", "g-f1", "g-f2"]);
    // a free item moves among the free ones; a steer among the next ones
    assert_eq!(mv(&s, "g-f2", Some("g-f1")).status, 200);
    assert_eq!(mv(&s, "g-n1", Some("g-s1")).status, 200);
    assert_eq!(waiting(&s, &ids), ["g-n1", "g-s1", "g-n2", "g-f2", "g-f1"]);
    // a move that cannot change anything says so in words, and is never `moved: true`
    let n = writes(&state);
    let a = mv(&s, "g-n1", Some("g-s1"));
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.json()["moved"], false);
    assert!(
        a.json()["message"]
            .as_str()
            .unwrap()
            .contains("nothing was written")
    );
    assert_eq!(writes(&state), n);
}

/// `steer`, `next` and `free` after moves: the list, the header the agent reads, and the
/// order after a restart all agree. A free item is read in free time (this host is not
/// committed), after the items read at the same boundary.
#[test]
fn steer_next_and_free_are_read_in_the_listed_order_after_moves_and_a_restart() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s23-intents-moves");
    let (state, inbox) = (guard.path().join("state"), guard.path().join("inbox"));
    let ids = ["m-n1", "m-f1", "m-s1", "m-n2", "m-f2"];
    let shown;
    {
        let s = quota(&state, &inbox);
        for (k, intent) in [
            ("m-n1", "next"),
            ("m-f1", "free"),
            ("m-s1", "steer"),
            ("m-n2", "next"),
            ("m-f2", "free"),
        ] {
            assert_eq!(add_intent(&s, k, intent).status, 201);
        }
        assert_eq!(waiting(&s, &ids), ["m-n1", "m-s1", "m-n2", "m-f1", "m-f2"]);
        assert_eq!(mv(&s, "m-s1", Some("m-n1")).status, 200);
        assert_eq!(mv(&s, "m-f2", Some("m-f1")).status, 200);
        assert_eq!(mv(&s, "m-n1", None).status, 200);
        shown = waiting(&s, &ids);
        assert_eq!(shown, ["m-s1", "m-n2", "m-n1", "m-f2", "m-f1"]);
    }
    {
        let s = quota(&state, &inbox);
        assert_eq!(waiting(&s, &ids), shown, "the order survives a restart");
    }
    let s = spawn(Kind::Plain, &state, &inbox, &[]);
    until(30, "all five to be answered", || {
        ids.iter().all(|k| item(&s, k)["state"] == "answered")
    });
    assert_eq!(read_in_headers(&state, &ids), shown);
    let read: Vec<String> = of_kind(&state, "stimulus.admitted")
        .iter()
        .flat_map(|l| {
            l.get("ids")
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
        })
        .filter(|id| ids.contains(&id.as_str()))
        .collect();
    assert_eq!(read, shown, "what the record says was admitted");
}
