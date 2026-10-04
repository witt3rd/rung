//! A turn does not hold the ACP connection: `session/cancel`,
//! `session/close` and other requests land while a prompt is running, on
//! both transports (stdio `--acp`, Streamable HTTP `--acp-http`).
//!
//! The mock LLM holds its first reply long enough for the client to act
//! mid-turn. A cancel that lands stops the turn at the next check (after the
//! LLM call, before the tool), so the mock sees exactly one request.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// How long the mock holds a slow reply; the client acts at `ACT`.
const HOLD: Duration = Duration::from_millis(2000);
const ACT: Duration = Duration::from_millis(300);

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rung-agent"))
}

fn tempdir() -> rung_testkit::TempDir {
    rung_testkit::TempDir::new("agent-acp-conc")
}

/// The scripted mock with a hold per reply; records each request body and
/// when it arrived.
fn mock_llm(replies: Vec<(Duration, Value)>) -> (String, Receiver<(Value, Instant)>) {
    let (tx, rx) = channel();
    let replies = replies
        .into_iter()
        .map(|(delay, mut reply)| {
            reply["__delay_ms"] = json!(delay.as_millis() as u64);
            reply
        })
        .collect();
    let url = rung_testkit::llm::serve_llm(replies, move |r| {
        let _ = tx.send((r.json(), r.at));
    });
    (url, rx)
}

fn text_reply(text: &str) -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": text}, "finish_reason": "stop"}]})
}

fn list_files_call() -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": "list_files", "arguments": "{\"path\":\".\"}"}}
    ]}, "finish_reason": "tool_calls"}]})
}

/// A slow tool call, then a fast answer. A turn that is not cancelled
/// makes two requests; one cancelled mid-call makes one.
fn slow_tool_then_answer() -> Vec<(Duration, Value)> {
    vec![
        (HOLD, list_files_call()),
        (Duration::ZERO, text_reply("done")),
    ]
}

fn agent(tmp: &Path, url: &str) -> Command {
    let mut cmd = bin();
    cmd.current_dir(tmp)
        .env("HOME", tmp)
        .env("XDG_CONFIG_HOME", tmp)
        .env("RUNG_CONFIG", tmp.join("none.yaml"))
        .env("RUNG_HOME", tmp)
        .env("RUNG_BASE_URL", url)
        .env("RUNG_MODEL", "m")
        .env("RUNG_API_KEY", "k")
        .env("RUNG_PROTOCOL", "openai")
        .env_remove("RUNG_KEY_FILE")
        .env_remove("RUNG_SYSTEM_PROMPT_FILE");
    cmd
}

/// Requests the mock saw, waiting a little for a late one.
fn seen(bodies: &Receiver<(Value, Instant)>) -> usize {
    let mut n = 0;
    while bodies.recv_timeout(Duration::from_millis(500)).is_ok() {
        n += 1;
    }
    n
}

// ---------------------------------------------------------------- stdio

struct Stdio1 {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    tmp: rung_testkit::TempDir,
}

impl Drop for Stdio1 {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Stdio1 {
    fn spawn(replies: Vec<(Duration, Value)>) -> (Self, Receiver<(Value, Instant)>) {
        let tmp = tempdir();
        let (url, bodies) = mock_llm(replies);
        let mut child = agent(&tmp, &url)
            .arg("--acp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        (
            Self {
                child,
                stdin,
                stdout,
                tmp,
            },
            bodies,
        )
    }

    fn send(&mut self, msg: Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, id: u32, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
    }

    /// The next response (not a notification).
    fn response(&mut self) -> Value {
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).expect("stdout line");
            assert!(!line.is_empty(), "agent closed stdout");
            let v: Value = serde_json::from_str(line.trim()).unwrap();
            if v.get("method").is_none() {
                return v;
            }
        }
    }

    fn ask(&mut self, id: u32, method: &str, params: Value) -> Value {
        self.request(id, method, params);
        let v = self.response();
        assert_eq!(v["id"], id, "{v}");
        v
    }

    /// initialize + session/new; the session id.
    fn open(&mut self) -> String {
        self.ask(1, "initialize", json!({"protocolVersion": 1}));
        self.new_session(2)
    }

    fn new_session(&mut self, id: u32) -> String {
        let cwd = self.tmp.to_string_lossy().into_owned();
        let created = self.ask(id, "session/new", json!({"cwd": cwd, "mcpServers": []}));
        created["result"]["sessionId"].as_str().unwrap().to_string()
    }

    /// The next `n` responses, ordered by id.
    fn responses(&mut self, n: usize) -> Vec<Value> {
        let mut got: Vec<Value> = (0..n).map(|_| self.response()).collect();
        got.sort_by_key(|v| v["id"].as_u64());
        got
    }
}

fn prompt(sid: &str, text: &str) -> Value {
    json!({"sessionId": sid, "prompt": [{"type": "text", "text": text}]})
}

/// `session/cancel` sent while the LLM call is open ends the turn as
/// `cancelled`, and the turn makes no further call.
#[test]
fn stdio_cancel_lands_mid_turn() {
    let (mut a, bodies) = Stdio1::spawn(slow_tool_then_answer());
    let sid = a.open();
    a.request(3, "session/prompt", prompt(&sid, "list"));
    std::thread::sleep(ACT);
    a.send(json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sid}}));
    let r = a.response();
    assert_eq!(r["id"], 3, "{r}");
    assert_eq!(r["result"]["stopReason"], "cancelled", "{r}");
    assert_eq!(seen(&bodies), 1, "the cancelled turn called the LLM again");
}

/// Requests are answered while a turn runs; the turn's own response comes
/// last.
#[test]
fn stdio_requests_answered_mid_turn() {
    let (mut a, _bodies) = Stdio1::spawn(vec![(HOLD, text_reply("late"))]);
    let sid = a.open();
    let cwd = a.tmp.to_string_lossy().into_owned();
    a.request(3, "session/prompt", prompt(&sid, "slow"));
    std::thread::sleep(ACT);
    a.request(4, "session/list", json!({"cwd": cwd}));
    let first = a.response();
    assert_eq!(first["id"], 4, "session/list waited for the turn: {first}");
    let r = a.response();
    assert_eq!(r["id"], 3, "{r}");
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
}

/// `session/close` mid-turn cancels the turn, and the closed status
/// survives the turn's own final write.
#[test]
fn stdio_close_cancels_mid_turn() {
    let (mut a, bodies) = Stdio1::spawn(slow_tool_then_answer());
    let sid = a.open();
    a.request(3, "session/prompt", prompt(&sid, "list"));
    std::thread::sleep(ACT);
    a.request(4, "session/close", json!({"sessionId": sid}));
    let got = a.responses(2);
    assert_eq!(got[0]["result"]["stopReason"], "cancelled", "{}", got[0]);
    assert!(got[1].get("result").is_some(), "{}", got[1]);
    assert_eq!(seen(&bodies), 1, "the closed turn called the LLM again");
    let file = a.tmp.join(".rung/sessions").join(format!("{sid}.json"));
    let sess: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    assert_eq!(sess["status"], "closed", "{sess}");
}

/// A cancel that arrives between turns does not poison the next turn.
#[test]
fn stdio_cancel_between_turns_is_a_no_op() {
    let (mut a, _bodies) = Stdio1::spawn(vec![(Duration::ZERO, text_reply("ok"))]);
    let sid = a.open();
    a.send(json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sid}}));
    let r = a.ask(3, "session/prompt", prompt(&sid, "go"));
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
}

/// Turns off the loop still run one at a time, in arrival order: the second
/// session's turn reaches the LLM only after the first's reply. (A turn sets
/// the process cwd; two at once would race it.)
#[test]
fn stdio_turns_stay_serialized() {
    let (mut a, bodies) = Stdio1::spawn(vec![
        (HOLD, text_reply("first")),
        (Duration::ZERO, text_reply("second")),
    ]);
    let s1 = a.open();
    let s2 = a.new_session(3);
    a.request(4, "session/prompt", prompt(&s1, "one"));
    a.request(5, "session/prompt", prompt(&s2, "two"));
    for r in a.responses(2) {
        assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    }
    let (first, t1) = bodies.recv().unwrap();
    let (second, t2) = bodies.recv().unwrap();
    assert!(first.to_string().contains("one"), "{first}");
    assert!(second.to_string().contains("two"), "{second}");
    assert!(
        t2.duration_since(t1) >= HOLD,
        "the second turn overlapped the first: {:?}",
        t2.duration_since(t1)
    );
}

/// A cancel stops a turn still waiting its place as well as the running
/// one; neither reaches the LLM again.
#[test]
fn stdio_cancel_reaches_a_queued_turn() {
    let (mut a, bodies) = Stdio1::spawn(slow_tool_then_answer());
    let sid = a.open();
    a.request(3, "session/prompt", prompt(&sid, "list"));
    a.request(4, "session/prompt", prompt(&sid, "and then"));
    std::thread::sleep(ACT);
    a.send(json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sid}}));
    for r in a.responses(2) {
        assert_eq!(r["result"]["stopReason"], "cancelled", "{r}");
    }
    assert_eq!(seen(&bodies), 1, "a cancelled turn called the LLM");
}

// ----------------------------------------------------------------- HTTP

struct Http1 {
    child: Child,
    addr: String,
    tmp: rung_testkit::TempDir,
}

impl Drop for Http1 {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Http1 {
    fn spawn(replies: Vec<(Duration, Value)>) -> (Self, Receiver<(Value, Instant)>) {
        let tmp = tempdir();
        let (url, bodies) = mock_llm(replies);
        let mut child = agent(&tmp, &url)
            .args(["--acp-http", "127.0.0.1:0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stderr = BufReader::new(child.stderr.take().unwrap());
        let mut line = String::new();
        stderr.read_line(&mut line).expect("listen line");
        let addr = line
            .split(" at http://")
            .nth(1)
            .and_then(|s| s.trim().strip_suffix("/acp"))
            .unwrap_or_else(|| panic!("listen line: {line}"))
            .to_string();
        // Keep stderr open: the agent writes to it during a turn.
        std::thread::spawn(move || std::io::copy(&mut stderr, &mut std::io::sink()));
        (Self { child, addr, tmp }, bodies)
    }

    fn connect(&self) -> TcpStream {
        let s = TcpStream::connect(&self.addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        s
    }

    /// POST a JSON-RPC message; the status and headers (body read only for
    /// a 200).
    fn post(&self, headers: &[(&str, &str)], body: &Value) -> (u16, Vec<(String, String)>, Value) {
        let bytes = serde_json::to_vec(body).unwrap();
        let mut req = format!(
            "POST /acp HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            self.addr,
            bytes.len()
        );
        for (k, v) in headers {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
        req.push_str("\r\n");
        let mut s = self.connect();
        s.write_all(req.as_bytes()).unwrap();
        s.write_all(&bytes).unwrap();
        let mut r = BufReader::new(s);
        let (status, hdrs) = head(&mut r);
        let mut body = Vec::new();
        let _ = r.read_to_end(&mut body);
        let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
        (status, hdrs, json)
    }

    fn sse(&self, headers: &[(&str, &str)]) -> BufReader<TcpStream> {
        let mut req = format!(
            "GET /acp HTTP/1.0\r\nHost: {}\r\nAccept: text/event-stream\r\n",
            self.addr
        );
        for (k, v) in headers {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
        req.push_str("\r\n");
        let mut s = self.connect();
        s.write_all(req.as_bytes()).unwrap();
        let mut r = BufReader::new(s);
        let (status, _) = head(&mut r);
        assert_eq!(status, 200);
        r
    }
}

fn head(r: &mut BufReader<TcpStream>) -> (u16, Vec<(String, String)>) {
    let mut line = String::new();
    r.read_line(&mut line).unwrap();
    let status = line.split_whitespace().nth(1).unwrap().parse().unwrap();
    let mut hdrs = Vec::new();
    loop {
        let mut l = String::new();
        r.read_line(&mut l).unwrap();
        if l.trim().is_empty() {
            return (status, hdrs);
        }
        if let Some((k, v)) = l.split_once(':') {
            hdrs.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
}

/// The next SSE event that is a response (not a notification).
fn sse_response(r: &mut BufReader<TcpStream>) -> Value {
    let mut data = String::new();
    loop {
        let mut line = String::new();
        r.read_line(&mut line).expect("sse line");
        if let Some(rest) = line.strip_prefix("data:") {
            data.push_str(rest.trim());
        } else if line.trim().is_empty() && !data.is_empty() {
            let v: Value = serde_json::from_str(&data).unwrap();
            data.clear();
            if v.get("method").is_none() {
                return v;
            }
        }
    }
}

/// The same cancel, over `--acp-http`.
#[test]
fn http_cancel_lands_mid_turn() {
    let (srv, bodies) = Http1::spawn(slow_tool_then_answer());
    let cwd = srv.tmp.to_string_lossy().into_owned();
    let (st, hdrs, _) = srv.post(
        &[],
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1}}),
    );
    assert_eq!(st, 200);
    let conn = hdrs
        .iter()
        .find(|(k, _)| k == "acp-connection-id")
        .map(|(_, v)| v.clone())
        .expect("Acp-Connection-Id");
    let mut conn_sse = srv.sse(&[("Acp-Connection-Id", &conn)]);
    let (st, _, _) = srv.post(
        &[("Acp-Connection-Id", &conn)],
        &json!({"jsonrpc": "2.0", "id": 2, "method": "session/new", "params": {"cwd": cwd, "mcpServers": []}}),
    );
    assert_eq!(st, 202);
    let created = sse_response(&mut conn_sse);
    let sid = created["result"]["sessionId"].as_str().unwrap().to_string();
    let both = [
        ("Acp-Connection-Id", conn.as_str()),
        ("Acp-Session-Id", &sid),
    ];
    let mut sess_sse = srv.sse(&both);
    let (st, _, _) = srv.post(
        &both,
        &json!({"jsonrpc": "2.0", "id": 3, "method": "session/prompt", "params": prompt(&sid, "list")}),
    );
    assert_eq!(st, 202);
    std::thread::sleep(ACT);
    let (st, _, _) = srv.post(
        &both,
        &json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sid}}),
    );
    assert_eq!(st, 202);
    let r = sse_response(&mut sess_sse);
    assert_eq!(r["id"], 3, "{r}");
    assert_eq!(r["result"]["stopReason"], "cancelled", "{r}");
    assert_eq!(seen(&bodies), 1, "the cancelled turn called the LLM again");
}
