//! `--acp` runs one worker process per session: a long turn in one session
//! does not hold another, and a killed worker takes only its own session.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const HOLD: Duration = Duration::from_millis(4000);

fn text_reply(text: &str) -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": text}, "finish_reason": "stop"}]})
}

struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<Value>,
    tmp: rung_testkit::TempDir,
}

impl Drop for Client {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Client {
    fn spawn(replies: Vec<(Duration, Value)>) -> Self {
        let tmp = rung_testkit::TempDir::new("agent-acp-workers");
        let replies = replies
            .into_iter()
            .map(|(d, mut r)| {
                r["__delay_ms"] = json!(d.as_millis() as u64);
                r
            })
            .collect();
        let url = rung_testkit::llm::serve_llm(replies, |_| {});
        let mut child = Command::new(env!("CARGO_BIN_EXE_rung-agent"))
            .arg("--acp")
            .current_dir(&tmp)
            .env("HOME", &tmp)
            .env("XDG_CONFIG_HOME", &tmp)
            .env("RUNG_CONFIG", tmp.join("none.yaml"))
            .env("RUNG_HOME", &tmp)
            .env("RUNG_BASE_URL", url)
            .env("RUNG_MODEL", "m")
            .env("RUNG_API_KEY", "k")
            .env("RUNG_PROTOCOL", "openai")
            .env_remove("RUNG_ACP_WORKERS")
            .env_remove("RUNG_ACP_WORKER")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let out: ChildStdout = child.stdout.take().unwrap();
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for l in BufReader::new(out).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<Value>(&l) {
                    let _ = tx.send(v);
                }
            }
        });
        Client {
            child,
            stdin,
            lines,
            tmp,
        }
    }

    fn send(&mut self, msg: Value) {
        let s = self.stdin.as_mut().unwrap();
        writeln!(s, "{msg}").unwrap();
        s.flush().unwrap();
    }

    fn request(&mut self, id: u32, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
    }

    /// The next message, within `wait`.
    fn next(&mut self, wait: Duration) -> Option<Value> {
        self.lines.recv_timeout(wait).ok()
    }

    /// The response to `id`, skipping notifications and other ids; the
    /// notifications seen on the way come back too.
    fn response(&mut self, id: u32, wait: Duration) -> (Value, Vec<Value>) {
        let end = Instant::now() + wait;
        let mut seen = Vec::new();
        loop {
            let left = end.saturating_duration_since(Instant::now());
            let v = self
                .next(left)
                .unwrap_or_else(|| panic!("no response to {id}; saw {seen:?}"));
            if v.get("id") == Some(&json!(id)) && v.get("method").is_none() {
                return (v, seen);
            }
            seen.push(v);
        }
    }

    fn new_session(&mut self, id: u32) -> String {
        let cwd = self.tmp.to_string_lossy().into_owned();
        self.request(id, "session/new", json!({"cwd": cwd, "mcpServers": []}));
        let (r, _) = self.response(id, Duration::from_secs(20));
        r["result"]["sessionId"]
            .as_str()
            .expect("session id")
            .to_string()
    }

    fn open2(&mut self) -> (String, String) {
        self.request(1, "initialize", json!({"protocolVersion": 1}));
        self.response(1, Duration::from_secs(20));
        let a = self.new_session(2);
        let b = self.new_session(3);
        (a, b)
    }
}

fn prompt(sid: &str, text: &str) -> Value {
    json!({"sessionId": sid, "prompt": [{"type": "text", "text": text}]})
}

/// Worker processes: direct children of `parent` running `rung-agent`.
fn workers_of(parent: u32) -> Vec<u32> {
    let mut v = Vec::new();
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        let after = stat.rsplit(')').next().unwrap_or("");
        let ppid: u32 = after
            .split_whitespace()
            .nth(1)
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);
        if ppid == parent {
            v.push(pid);
        }
    }
    v.sort();
    v
}

fn kill9(pid: u32) {
    let st = Command::new("sh")
        .args(["-c", &format!("kill -9 {pid}")])
        .status()
        .unwrap();
    assert!(st.success());
}

fn rss_kb(pid: u32) -> u64 {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

/// While session A runs a long call, session B answers at once.
#[test]
fn second_session_is_not_held_by_the_first() {
    let mut c = Client::spawn(vec![
        (HOLD, text_reply("slow")),
        (Duration::ZERO, text_reply("fast")),
    ]);
    let (a, b) = c.open2();
    c.request(10, "session/prompt", prompt(&a, "one"));
    std::thread::sleep(Duration::from_millis(500));
    let t = Instant::now();
    c.request(11, "session/prompt", prompt(&b, "two"));
    let (r, _) = c.response(11, Duration::from_secs(3));
    assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    let (r, _) = c.response(10, Duration::from_secs(10));
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
}

/// Each session has its own process; kill -9 of one leaves the other
/// answering, and the killed session comes back on its next prompt.
#[test]
fn killing_one_worker_leaves_the_others() {
    let mut c = Client::spawn(vec![
        (Duration::ZERO, text_reply("a1")),
        (Duration::ZERO, text_reply("b1")),
        (Duration::ZERO, text_reply("a2")),
        (Duration::ZERO, text_reply("b2")),
    ]);
    let (a, b) = c.open2();
    let pids = workers_of(c.child.id());
    assert_eq!(pids.len(), 2, "one worker per session: {pids:?}");
    for p in &pids {
        eprintln!("worker {p} idle rss: {} kB", rss_kb(*p));
    }
    // The first worker answered initialize and took session a.
    kill9(pids[0]);
    std::thread::sleep(Duration::from_millis(300));

    c.request(10, "session/prompt", prompt(&b, "still there?"));
    let (r, _) = c.response(10, Duration::from_secs(5));
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");

    // After the 250 ms backoff the killed session restarts from its file.
    std::thread::sleep(Duration::from_millis(400));
    c.request(11, "session/prompt", prompt(&a, "back?"));
    let (r, _) = c.response(11, Duration::from_secs(10));
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    assert_eq!(workers_of(c.child.id()).len(), 2);
}

/// A worker killed mid-turn fails that turn's request; the other session
/// keeps its stream.
#[test]
fn a_turn_open_when_its_worker_dies_gets_an_error() {
    let mut c = Client::spawn(vec![
        (HOLD, text_reply("slow")),
        (Duration::ZERO, text_reply("fast")),
    ]);
    let (a, b) = c.open2();
    c.request(10, "session/prompt", prompt(&a, "one"));
    std::thread::sleep(Duration::from_millis(500));
    let pids = workers_of(c.child.id());
    kill9(pids[0]);
    let (r, _) = c.response(10, Duration::from_secs(3));
    assert!(r.get("error").is_some(), "{r}");
    c.request(11, "session/prompt", prompt(&b, "two"));
    let (r, _) = c.response(11, Duration::from_secs(5));
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
}

/// Streaming and cancel cross the process boundary: updates arrive before
/// the response, and a cancel ends the turn.
#[test]
fn streaming_and_cancel_cross_the_boundary() {
    let mut c = Client::spawn(vec![(Duration::ZERO, text_reply("hello"))]);
    let (a, _b) = c.open2();
    c.request(10, "session/prompt", prompt(&a, "hi"));
    let (r, seen) = c.response(10, Duration::from_secs(10));
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    assert!(
        seen.iter().any(|m| m["method"] == "session/update"),
        "no session/update before the response: {seen:?}"
    );

    let mut c = Client::spawn(vec![(HOLD, text_reply("slow"))]);
    let (a, _b) = c.open2();
    c.request(10, "session/prompt", prompt(&a, "slow"));
    std::thread::sleep(Duration::from_millis(500));
    c.send(json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": a}}));
    let (r, _) = c.response(10, Duration::from_secs(10));
    assert_eq!(r["result"]["stopReason"], "cancelled", "{r}");
}
