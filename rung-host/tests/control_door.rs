//! S7: the owner's `POST /v1/stop` and `POST /v1/release` on the host's own
//! listener, over real HTTP against the `rung-host` binary (`sim --acp-http`,
//! real clock, mock engine). Written first against a host that had neither door.
//!
//! `wedge` holds a first turn that does not end, so a host that has been told to
//! stop is still up to be asked again; `quota` is a host that waits.

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
    if !extra.contains(&"--p-commit") {
        args.push("--no-commit");
    }
    match kind {
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

fn lines(state: &Path) -> Vec<Line> {
    Record::read_dir(state.join("record")).unwrap_or_default()
}

fn of_kind(state: &Path, kind: &str) -> Vec<Line> {
    lines(state)
        .into_iter()
        .filter(|l| l.kind == kind)
        .collect()
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

fn post(s: &Server, token: Option<&str>, path: &str, body: Option<&str>) -> Answer {
    let mut rb = client().post(format!("{}{path}", s.base));
    if let Some(b) = body {
        rb = rb
            .header("content-type", "application/json")
            .body(b.to_string());
    }
    answer(match token {
        Some(t) => rb.bearer_auth(t),
        None => rb,
    })
}

fn controls(state: &Path, control: &str) -> usize {
    of_kind(state, "stimulus.accepted")
        .iter()
        .filter(|l| l.get("item")["control"] == control)
        .count()
}

#[test]
fn the_owner_stops_the_host_and_the_stop_is_in_the_record_before_the_answer() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-stop");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let mut s = spawn(Kind::Plain, &state, &inbox, &[]);
    let a = post(&s, Some(OWNER_TOKEN), "/v1/stop", None);
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.json()["stopping"], true);
    assert_eq!(a.json()["repeat"], false);
    assert_eq!(controls(&state, "stop"), 1);
    until(20, "the host to exit", || {
        s.child.try_wait().unwrap().is_some()
    });
    assert!(s.child.wait().unwrap().success());
}

#[test]
fn a_second_stop_is_a_repeat_and_writes_nothing() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-stop-twice");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let s = wedged(&state, &inbox);
    let first = post(&s, Some(OWNER_TOKEN), "/v1/stop", None);
    assert_eq!(first.status, 200, "{}", first.body);
    let second = post(&s, Some(OWNER_TOKEN), "/v1/stop", None);
    assert_eq!(second.status, 200, "{}", second.body);
    assert_eq!(second.json()["stopping"], true);
    assert_eq!(second.json()["repeat"], true);
    assert_eq!(controls(&state, "stop"), 1);
}

#[test]
fn only_the_owner_may_stop_or_release_and_a_refusal_writes_nothing() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-roles");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let s = wedged(&state, &inbox);
    for path in ["/v1/stop", "/v1/release"] {
        assert_eq!(post(&s, None, path, None).status, 401);
        assert_eq!(post(&s, Some("not-a-token"), path, None).status, 401);
        let peer = post(&s, Some(PEER_TOKEN), path, None);
        assert_eq!(peer.status, 403, "{}", peer.body);
        assert_eq!(peer.json()["error"], "forbidden");
        assert_eq!(
            client()
                .get(format!("{}{path}", s.base))
                .bearer_auth(OWNER_TOKEN)
                .send()
                .unwrap()
                .status(),
            405
        );
    }
    assert_eq!(controls(&state, "stop") + controls(&state, "release"), 0);
    // still running: the owner's queue door answers
    assert_eq!(get(&s, "/v1/queue").status, 200);
}

#[test]
fn a_release_with_nothing_held_is_a_no_op_that_says_so_and_writes_nothing() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-release");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let s = spawn(Kind::Plain, &state, &inbox, &["--no-commit"]);
    for _ in 0..2 {
        let a = post(&s, Some(OWNER_TOKEN), "/v1/release", Some("{}"));
        assert_eq!(a.status, 200, "{}", a.body);
        assert_eq!(a.json()["released"], false);
        assert_eq!(a.json()["repeat"], true);
    }
    let with_reason = post(
        &s,
        Some(OWNER_TOKEN),
        "/v1/release",
        Some(&json!({"reason": "enough"}).to_string()),
    );
    assert_eq!(with_reason.status, 200, "{}", with_reason.body);
    assert_eq!(controls(&state, "release"), 0);
}

#[test]
fn a_body_that_is_not_what_the_door_takes_is_refused_by_name() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-body");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let s = spawn(Kind::Plain, &state, &inbox, &["--no-commit"]);
    let a = post(&s, Some(OWNER_TOKEN), "/v1/release", Some("{\"why\": 1}"));
    assert_eq!(a.status, 400);
    assert!(a.message().contains("why"), "{}", a.message());
    let b = post(&s, Some(OWNER_TOKEN), "/v1/release", Some("[1]"));
    assert_eq!(b.status, 400);
    let c = post(
        &s,
        Some(OWNER_TOKEN),
        "/v1/release",
        Some("{\"reason\": 5}"),
    );
    assert_eq!(c.status, 400);
    let e = post(&s, Some(OWNER_TOKEN), "/v1/stop", Some("{\"x\": 1}"));
    assert_eq!(e.status, 400);
    assert_eq!(controls(&state, "stop") + controls(&state, "release"), 0);
}

#[test]
fn a_release_to_a_stopping_host_is_refused_in_words() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-release-stopping");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let s = wedged(&state, &inbox);
    assert_eq!(post(&s, Some(OWNER_TOKEN), "/v1/stop", None).status, 200);
    let a = post(&s, Some(OWNER_TOKEN), "/v1/release", Some("{}"));
    assert_eq!(a.status, 409, "{}", a.body);
    assert_eq!(a.json()["error"], "not_running");
    assert!(a.message().contains("start it first"), "{}", a.message());
    assert_eq!(controls(&state, "release"), 0);
}

/// Twenty owners press stop at once: one line in the record, nineteen repeats.
#[test]
fn twenty_concurrent_stops_make_exactly_one_line_and_nineteen_repeats() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-stop-race");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let s = wedged(&state, &inbox);
    let answers: Vec<Answer> = std::thread::scope(|t| {
        let hs: Vec<_> = (0..20)
            .map(|_| t.spawn(|| post(&s, Some(OWNER_TOKEN), "/v1/stop", None)))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(answers.iter().all(|a| a.status == 200));
    let repeats = answers
        .iter()
        .filter(|a| a.json()["repeat"] == true)
        .count();
    assert_eq!(repeats, 19);
    assert_eq!(controls(&state, "stop"), 1);
    let disposed = of_kind(&state, "stimulus.disposed")
        .iter()
        .filter(|l| l.str("disposition") == "control")
        .count();
    assert_eq!(disposed, 1);
}

/// Twenty releases at once with nothing held: no line, twenty repeats.
#[test]
fn twenty_concurrent_releases_with_nothing_held_write_nothing() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-release-race");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let s = spawn(Kind::Plain, &state, &inbox, &["--no-commit"]);
    let answers: Vec<Answer> = std::thread::scope(|t| {
        let hs: Vec<_> = (0..20)
            .map(|_| t.spawn(|| post(&s, Some(OWNER_TOKEN), "/v1/release", Some("{}"))))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(
        answers
            .iter()
            .all(|a| a.status == 200 && a.json()["repeat"] == true)
    );
    assert_eq!(controls(&state, "release"), 0);
}

/// A restarted host on the same state is running again (the stop does not
/// outlive the process that took it) and takes a new stop as a first one.
#[test]
fn a_restart_keeps_one_stop_line_and_a_new_stop_is_a_first_one() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-stop-restart");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let mut s = spawn(Kind::Plain, &state, &inbox, &[]);
    assert_eq!(post(&s, Some(OWNER_TOKEN), "/v1/stop", None).status, 200);
    until(20, "the host to exit", || {
        s.child.try_wait().unwrap().is_some()
    });
    assert_eq!(controls(&state, "stop"), 1);
    drop(s);
    let s2 = spawn(Kind::Plain, &state, &inbox, &[]);
    assert_eq!(get(&s2, "/v1/queue").status, 200);
    assert_eq!(controls(&state, "stop"), 1);
    let a = post(&s2, Some(OWNER_TOKEN), "/v1/stop", None);
    assert_eq!(a.json()["repeat"], false, "{}", a.body);
    assert_eq!(controls(&state, "stop"), 2);
}

/// The owner kernel releases in the record.
fn owner_releases(state: &Path) -> usize {
    of_kind(state, "kernel.release")
        .iter()
        .filter(|l| l.str("released_by") == "owner")
        .count()
}

/// A host that commits to a project on its first free turn (and again after a
/// release; the mock decides on every uncommitted free turn).
fn committing(state: &Path, inbox: &Path) -> Server {
    let s = spawn(Kind::Plain, state, inbox, &["--p-commit", "1.0"]);
    until(20, "the host to commit", || {
        !of_kind(state, "kernel.commit").is_empty()
    });
    s
}

/// The thing the release door is for: a held commitment is let go. The effect
/// is in the record (`kernel.release`, by the owner, with the reason, once), not
/// only in the answer.
#[test]
fn a_release_lets_go_of_the_held_commitment() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-release-effect");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let s = committing(&state, &inbox);
    // the mock also ends a project by itself, so press until a press finds one
    // held; a door that never releases never gets there
    let mut released = false;
    for _ in 0..200 {
        let a = post(
            &s,
            Some(OWNER_TOKEN),
            "/v1/release",
            Some(&json!({"reason": "enough for now"}).to_string()),
        );
        assert_eq!(a.status, 200, "{}", a.body);
        if a.json()["released"] == true {
            assert_eq!(a.json()["repeat"], false);
            released = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(released, "no press ever released a held commitment");
    let l = of_kind(&state, "kernel.release")
        .into_iter()
        .find(|l| l.str("released_by") == "owner")
        .expect("the owner's release is in the record");
    assert_eq!(l.str("reason"), "enough for now");
    assert_eq!(l.str("via"), "owner:_rung/release");
    assert_eq!(owner_releases(&state), controls(&state, "release"));
}

/// Twenty releases at once: every line the doors wrote is one that released
/// (the answers, the control lines and the owner's `kernel.release` lines agree),
/// never a line without a release.
#[test]
fn concurrent_releases_write_one_line_per_actual_release() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-release-concurrent");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let s = committing(&state, &inbox);
    let answers: Vec<Answer> = std::thread::scope(|t| {
        let hs: Vec<_> = (0..20)
            .map(|_| t.spawn(|| post(&s, Some(OWNER_TOKEN), "/v1/release", Some("{}"))))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(answers.iter().all(|a| a.status == 200));
    let yes = answers
        .iter()
        .filter(|a| a.json()["released"] == true)
        .count();
    assert!(yes >= 1, "a held commitment was released");
    assert_eq!(controls(&state, "release"), yes);
    assert_eq!(owner_releases(&state), yes);
}

/// Stop takes no body: a declared one is refused from the headers, before it is
/// read, and nothing is written.
#[test]
fn a_stop_with_a_body_is_refused_before_the_body_is_read() {
    sim::test_timeout(120);
    let d = sim::temp_dir_guard("s7-stop-body");
    let (state, inbox) = (d.path().join("state"), d.path().join("inbox"));
    let s = spawn(Kind::Plain, &state, &inbox, &["--no-commit"]);
    // refused from the headers: the answer is 400, or the host has closed the
    // connection on the sender before the whole body went (also "not read")
    let big = "x".repeat(8 * 1024 * 1024);
    let sent = client()
        .post(format!("{}/v1/stop", s.base))
        .bearer_auth(OWNER_TOKEN)
        .header("content-type", "application/json")
        .body(big)
        .send();
    if let Ok(r) = sent {
        assert_eq!(r.status().as_u16(), 400);
    }
    let a = post(&s, Some(OWNER_TOKEN), "/v1/stop", Some("{}"));
    assert_eq!(a.status, 400, "{}", a.message());
    assert_eq!(controls(&state, "stop"), 0);
    assert_eq!(post(&s, Some(OWNER_TOKEN), "/v1/stop", None).status, 200);
}
