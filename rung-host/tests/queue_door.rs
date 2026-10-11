//! S1: the host's own `GET/POST /v1/queue` doors on its HTTP listener, over
//! real HTTP against the `rung-host` binary (`sim --acp-http`, real clock, mock
//! engine). Written first against a host that had neither door. No test sleeps
//! on a clock; a sentinel is never put in an assertion message.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use rung_host::record::{Line, Record};
use rung_host::sim;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");
const OWNER_TOKEN: &str = "s1-owner-token-canary-5521";
const PEER_TOKEN: &str = "s1-peer-token-canary-7743";
/// A sentinel with a known key shape: found by the redactor with no help.
const SHAPED: &str = "sk-SENTINELs1shaped0123456789abcdef";

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

fn spawn(state: &Path) -> Server {
    spawn_with(state, &[])
}

fn spawn_with(state: &Path, extra: &[&str]) -> Server {
    let mut child = Command::new(BIN)
        .args([
            "sim",
            "--clock",
            "real",
            "--state",
            state.to_str().unwrap(),
            "--call-ms",
            "20,40",
            "--no-commit",
            "--acp-http",
            "127.0.0.1:0",
            "--acp-token-env",
            "owner=S1_OWNER_TOKEN",
            "--acp-token-env",
            "peer=S1_PEER_TOKEN",
        ])
        .args(extra)
        .env("S1_OWNER_TOKEN", OWNER_TOKEN)
        .env("S1_PEER_TOKEN", PEER_TOKEN)
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
}

fn answer(rb: reqwest::blocking::RequestBuilder) -> Answer {
    let r = rb.send().unwrap();
    Answer {
        status: r.status().as_u16(),
        body: r.text().unwrap(),
    }
}

fn get(s: &Server, path: &str, token: Option<&str>) -> Answer {
    let rb = client().get(format!("{}{path}", s.base));
    answer(match token {
        Some(t) => rb.bearer_auth(t),
        None => rb,
    })
}

fn post(s: &Server, body: &Value, token: Option<&str>) -> Answer {
    let rb = client()
        .post(format!("{}/v1/queue", s.base))
        .header("content-type", "application/json")
        .body(body.to_string());
    answer(match token {
        Some(t) => rb.bearer_auth(t),
        None => rb,
    })
}

fn lines(state: &Path) -> Vec<Line> {
    Record::read_dir(state.join("record")).unwrap_or_default()
}

fn accepted<'a>(lines: &'a [Line], id: &str) -> Vec<&'a Line> {
    lines
        .iter()
        .filter(|l| l.kind == "stimulus.accepted" && l.get("item")["id"] == id)
        .collect()
}

#[test]
fn a_message_is_in_the_record_before_the_door_answers() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-durable");
    let s = spawn(guard.path());
    let a = post(
        &s,
        &json!({"text": "please check the Q3 numbers", "key": "k-durable"}),
        Some(OWNER_TOKEN),
    );
    assert_eq!(a.status, 201, "{}", a.body);
    let v = a.json();
    assert_eq!(v["id"], "k-durable");
    assert_eq!(v["repeat"], false);
    // the answer is the proof: the line is on disk by the time it is read
    let l = lines(guard.path());
    let got = accepted(&l, "k-durable");
    assert_eq!(got.len(), 1, "one stimulus.accepted line for the item");
    let item = got[0].get("item");
    assert_eq!(item["role"], "owner");
    assert_eq!(item["text"], "please check the Q3 numbers");
    // and the item is in the answer in the list's shape
    assert_eq!(v["item"]["id"], "k-durable");
    assert_eq!(v["item"]["text"], "please check the Q3 numbers");
    assert_eq!(v["item"]["role"], "owner");
    assert_eq!(v["item"]["intent"], "next");
}

#[test]
fn the_same_key_twice_is_one_item() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-repeat");
    let s = spawn(guard.path());
    let body = json!({"text": "once only", "key": "k-once"});
    let first = post(&s, &body, Some(OWNER_TOKEN));
    assert_eq!(first.status, 201, "{}", first.body);
    let again = post(&s, &body, Some(OWNER_TOKEN));
    assert_eq!(again.status, 200, "{}", again.body);
    let v = again.json();
    assert_eq!(v["id"], "k-once");
    assert_eq!(v["repeat"], true);
    assert_eq!(accepted(&lines(guard.path()), "k-once").len(), 1);
    let list = get(&s, "/v1/queue?state=all", Some(OWNER_TOKEN)).json();
    let n = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["id"] == "k-once")
        .count();
    assert_eq!(n, 1);
    // the same key for a different message is refused in words, nothing written
    let other = post(
        &s,
        &json!({"text": "a different message", "key": "k-once"}),
        Some(OWNER_TOKEN),
    );
    assert_eq!(other.status, 409, "{}", other.body);
    assert_eq!(other.json()["error"], "key_reused");
    assert!(other.json()["message"].as_str().unwrap().contains("key"));
    assert_eq!(accepted(&lines(guard.path()), "k-once").len(), 1);
}

#[test]
fn a_message_without_a_key_gets_its_own_id_each_time() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-nokey");
    let s = spawn(guard.path());
    let a = post(&s, &json!({"text": "same words"}), Some(OWNER_TOKEN)).json();
    let b = post(&s, &json!({"text": "same words"}), Some(OWNER_TOKEN)).json();
    assert_ne!(a["id"], b["id"]);
    assert_eq!(a["repeat"], false);
    assert_eq!(b["repeat"], false);
}

#[test]
fn a_200000_character_task_is_queued_and_listed_whole() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-whole");
    let s = spawn(guard.path());
    let text: String = (0..200_000)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    let a = post(
        &s,
        &json!({"text": text, "key": "k-big"}),
        Some(OWNER_TOKEN),
    );
    assert_eq!(a.status, 201);
    let list = get(&s, "/v1/queue?state=all", Some(OWNER_TOKEN)).json();
    let item = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "k-big")
        .unwrap()
        .clone();
    assert_eq!(item["text"].as_str().unwrap().len(), 200_000);
    assert!(item["text"].as_str().unwrap() == text, "listed whole");
    assert!(!item["text"].as_str().unwrap().contains('…'));
}

/// The item of `list` with this id.
fn item_of<'a>(list: &'a Value, id: &str) -> &'a Value {
    let mut it = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["id"] == id);
    let found = it.next().unwrap_or_else(|| panic!("no item {id}"));
    assert!(it.next().is_none(), "item {id} listed twice");
    found
}

/// A running host has items of its own besides the ones a test adds: its own
/// expectations settle into the queue once it has run free turns (`exp-…`, role
/// `host`), and how many have by the time a test looks depends on how long the
/// host has been running. So this test asserts only about the items it added,
/// by their keys, and about the page arithmetic with a bound that holds however
/// many others there are; exact totals and positions are the fold's and the
/// gateway's own tests', over fixed records.
#[test]
fn the_list_has_the_shape_a_page_needs() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-list");
    let s = spawn(guard.path());
    for n in 1..=4 {
        let a = post(
            &s,
            &json!({"text": format!("m{n}"), "key": format!("k-{n}")}),
            Some(OWNER_TOKEN),
        );
        assert_eq!(a.status, 201, "{}", a.body);
    }
    let list = get(&s, "/v1/queue?state=all", Some(OWNER_TOKEN));
    assert_eq!(list.status, 200, "{}", list.body);
    let v = list.json();
    assert!(v["total"].as_u64().unwrap() >= 4);
    assert_eq!(v["offset"], 0);
    let i = item_of(&v, "k-1");
    assert_eq!(i["text"], "m1");
    assert_eq!(i["role"], "owner");
    assert!(i["name"].is_null());
    assert_eq!(i["intent"], "next");
    assert!(i["order"].as_u64().unwrap() > 0);
    assert!(i["accepted_at"].as_i64().unwrap() > 0);
    assert!(matches!(
        i["state"].as_str().unwrap(),
        "waiting" | "answering" | "answered"
    ));
    // the owner's items are in the order they were added
    let order = |id: &str| item_of(&v, id)["order"].as_u64().unwrap();
    assert!(
        order("k-1") < order("k-2") && order("k-2") < order("k-3") && order("k-3") < order("k-4")
    );
    // paging: `limit` is the page, `next` the next offset, past the end empty
    let p = get(&s, "/v1/queue?state=all&limit=3", Some(OWNER_TOKEN)).json();
    assert_eq!(p["items"].as_array().unwrap().len(), 3);
    assert_eq!(p["limit"], 3);
    assert_eq!(p["next"], 3);
    let p = get(
        &s,
        "/v1/queue?state=all&limit=3&offset=3",
        Some(OWNER_TOKEN),
    )
    .json();
    let n = p["items"].as_array().unwrap().len();
    assert!((1..=3).contains(&n));
    assert_eq!(p["offset"], 3);
    let p = get(&s, "/v1/queue?state=all&offset=100000", Some(OWNER_TOKEN)).json();
    assert_eq!(p["items"], json!([]));
    assert!(p["next"].is_null());
    // a parameter it does not know, and a state it does not know, are refused
    let bad = get(&s, "/v1/queue?colour=red", Some(OWNER_TOKEN));
    assert_eq!(bad.status, 400);
    let bad = get(&s, "/v1/queue?state=sleeping", Some(OWNER_TOKEN));
    assert_eq!(bad.status, 400);
}

#[test]
fn only_the_owner_token_may_use_the_queue() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-roles");
    let s = spawn(guard.path());
    let body = json!({"text": "x", "key": "k-role"});
    assert_eq!(post(&s, &body, None).status, 401);
    assert_eq!(post(&s, &body, Some("not-a-token")).status, 401);
    let peer = post(&s, &body, Some(PEER_TOKEN));
    assert_eq!(peer.status, 403, "{}", peer.body);
    assert_eq!(get(&s, "/v1/queue", None).status, 401);
    assert_eq!(get(&s, "/v1/queue", Some(PEER_TOKEN)).status, 403);
    assert_eq!(get(&s, "/v1/queue", Some(OWNER_TOKEN)).status, 200);
    assert!(accepted(&lines(guard.path()), "k-role").is_empty());
    // another method is not this door's
    let d = answer(
        client()
            .put(format!("{}/v1/queue", s.base))
            .bearer_auth(OWNER_TOKEN),
    );
    assert_eq!(d.status, 405);
}

#[test]
fn a_bad_request_is_refused_in_words_and_writes_nothing() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-bad");
    let s = spawn(guard.path());
    let cases: Vec<(Value, &str)> = vec![
        (json!({"key": "k-a"}), "text"),
        (json!({"text": "", "key": "k-b"}), "empty"),
        (json!({"text": "   \n", "key": "k-c"}), "empty"),
        (json!({"text": 7, "key": "k-d"}), "text"),
        (json!({"text": "x", "key": ""}), "key"),
        (json!({"text": "x", "key": "has space"}), "key"),
        (json!({"text": "x", "key": "../up"}), "key"),
        (json!({"text": "x", "colour": "red"}), "colour"),
        (json!({"text": "x", "intent": "later"}), "intent"),
        (json!({"text": "x", "intent": "steer"}), "intent"),
        (json!({"text": "x", "intent": "free"}), "intent"),
    ];
    for (body, word) in cases {
        let a = post(&s, &body, Some(OWNER_TOKEN));
        assert_eq!(a.status, 400, "{body}: {}", a.body);
        let msg = a.json()["message"].as_str().unwrap().to_string();
        assert!(msg.contains(word), "{body}: {msg}");
    }
    // not JSON at all
    let r = answer(
        client()
            .post(format!("{}/v1/queue", s.base))
            .header("content-type", "application/json")
            .body("{not json")
            .bearer_auth(OWNER_TOKEN),
    );
    assert_eq!(r.status, 400);
    let r = answer(
        client()
            .post(format!("{}/v1/queue", s.base))
            .header("content-type", "text/plain")
            .body("hello")
            .bearer_auth(OWNER_TOKEN),
    );
    assert_eq!(r.status, 415);
    let l = lines(guard.path());
    assert!(
        !l.iter().any(|l| l.kind == "stimulus.accepted"
            && l.get("item")["role"] == "owner"
            && l.get("item")["channel"] == "owner"),
        "nothing was written for any refused request"
    );
    // the intent the door knows is accepted, spelled out
    let ok = post(
        &s,
        &json!({"text": "fine", "intent": "next", "key": "k-ok"}),
        Some(OWNER_TOKEN),
    );
    assert_eq!(ok.status, 201, "{}", ok.body);
}

#[test]
fn every_outgoing_text_is_redacted_and_the_record_keeps_the_original() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-redact");
    let s = spawn(guard.path());
    let text = format!("use {SHAPED} and also {OWNER_TOKEN} and {PEER_TOKEN}");
    let a = post(
        &s,
        &json!({"text": text, "key": "k-red"}),
        Some(OWNER_TOKEN),
    );
    assert_eq!(a.status, 201);
    let list = get(&s, "/v1/queue?state=all", Some(OWNER_TOKEN));
    for wire in [&a.body, &list.body] {
        assert!(!wire.contains(SHAPED), "a shaped secret reached the wire");
        assert!(
            !wire.contains(OWNER_TOKEN),
            "the owner token reached the wire"
        );
        assert!(!wire.contains(PEER_TOKEN), "a role token reached the wire");
        assert!(wire.contains("[redacted]"));
    }
    // the record is verbatim: what the agent reads is what was sent
    let l = lines(guard.path());
    let item = accepted(&l, "k-red")[0].get("item").clone();
    assert!(
        item["text"] == text.as_str(),
        "the record holds the original"
    );
}

#[test]
fn twenty_concurrent_posts_with_one_key_make_exactly_one_item() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-atomic");
    let s = spawn(guard.path());
    let base = s.base.clone();
    let go = std::sync::Barrier::new(20);
    let answers: Vec<(u16, bool)> = std::thread::scope(|t| {
        let hs: Vec<_> = (0..20)
            .map(|_| {
                t.spawn(|| {
                    go.wait();
                    let a = answer(
                        client()
                            .post(format!("{base}/v1/queue"))
                            .header("content-type", "application/json")
                            .body(json!({"text": "race", "key": "k-race"}).to_string())
                            .bearer_auth(OWNER_TOKEN),
                    );
                    (a.status, a.json()["repeat"] == true)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(answers.iter().filter(|(s, _)| *s == 201).count(), 1);
    assert_eq!(answers.iter().filter(|(s, r)| *s == 200 && *r).count(), 19);
    assert_eq!(accepted(&lines(guard.path()), "k-race").len(), 1);
}

#[test]
fn a_host_that_is_stopping_refuses_with_start_it_first_and_writes_nothing() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-stopping");
    let stop_file = guard.path().join("STOP");
    let s = spawn_with(guard.path(), &["--stop-file", stop_file.to_str().unwrap()]);
    let ok = post(
        &s,
        &json!({"text": "before", "key": "k-before"}),
        Some(OWNER_TOKEN),
    );
    assert_eq!(ok.status, 201, "{}", ok.body);
    std::fs::write(&stop_file, "stop").unwrap();
    // the host answers 409 from the moment the stop is raised until it exits
    let mut refused = None;
    for n in 0..400 {
        let r = client()
            .post(format!("{}/v1/queue", s.base))
            .header("content-type", "application/json")
            .body(json!({"text": "late", "key": format!("k-late-{n}")}).to_string())
            .bearer_auth(OWNER_TOKEN)
            .send();
        let Ok(r) = r else { break };
        let (status, body) = (r.status().as_u16(), r.text().unwrap_or_default());
        if status == 409 {
            refused = Some((n, body));
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let (n, body) = refused.expect("a 409 while the host was stopping");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["error"], "not_running");
    assert!(v["message"].as_str().unwrap().contains("start it first"));
    assert!(accepted(&lines(guard.path()), &format!("k-late-{n}")).is_empty());
}

#[test]
fn a_retry_after_the_host_restarts_is_still_one_item() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s1-queue-restart");
    let body = json!({"text": "survive a restart", "key": "k-restart"});
    {
        let s = spawn(guard.path());
        let a = post(&s, &body, Some(OWNER_TOKEN));
        assert_eq!(a.status, 201, "{}", a.body);
        // killed with no goodbye: the item was on disk before the answer
    }
    let s = spawn(guard.path());
    let again = post(&s, &body, Some(OWNER_TOKEN));
    assert_eq!(again.status, 200, "{}", again.body);
    assert_eq!(again.json()["repeat"], true);
    assert_eq!(accepted(&lines(guard.path()), "k-restart").len(), 1);
}
