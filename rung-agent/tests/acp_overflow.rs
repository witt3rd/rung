//! Context overflow over ACP, end to end against a mock model.
//!
//! A provider context overflow rolls the turn back (nothing of it is
//! stored, its ask included), the oldest tool results are elided once and
//! the call is retried once. A turn that still overflows ends in the typed
//! `overflow` state with the provider's own token figures on a
//! `usage_update`, and the session takes the next prompt.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::Receiver;

use serde_json::{Value, json};

/// What an elided tool result says in place of its content.
const ELIDED: &str = "[tool result elided";

fn tempdir() -> rung_testkit::TempDir {
    rung_testkit::TempDir::new("agent-overflow")
}

use rung_testkit::llm::mock_llm;

fn text_reply(text: &str) -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": text}, "finish_reason": "stop"}]})
}

fn read_call(id: &str, path: &str) -> Value {
    let args = json!({ "path": path }).to_string();
    json!({"id": "c", "model": "m", "choices": [{"message": {"tool_calls": [
        {"id": id, "type": "function", "function": {"name": "read_file", "arguments": args}}
    ]}, "finish_reason": "tool_calls"}]})
}

/// The provider's 400 for a request over the window, with its figures.
fn overflow() -> Value {
    json!({"__status": 400, "__body": {"error": {
        "message": "This model's maximum context length is 8192 tokens. However, your messages resulted in 9100 tokens. Please reduce the length of the messages.",
        "code": "context_length_exceeded",
    }}})
}

/// A file whose read is a large tool result. Every line carries `tag`.
fn big_file(dir: &std::path::Path, name: &str, tag: &str) {
    let text: String = (0..60)
        .map(|i| format!("{tag}-{i:03} lorem ipsum dolor sit amet\n"))
        .collect();
    std::fs::write(dir.join(name), text).unwrap();
}

/// One `rung-agent --acp --tools read` process on a fresh session.
struct Acp {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    dir: rung_testkit::TempDir,
    sid: String,
    next_id: u32,
    bodies: Receiver<Value>,
}

/// One prompt's response and the `session/update`s sent before it.
struct Answer {
    response: Value,
    updates: Vec<Value>,
}

impl Acp {
    fn start(dir: rung_testkit::TempDir, replies: Vec<Value>) -> Acp {
        let (url, bodies) = mock_llm(replies);
        let mut child = Command::new(env!("CARGO_BIN_EXE_rung-agent"))
            .args(["--acp", "--tools", "read"])
            .current_dir(&dir)
            .env("HOME", &dir)
            .env("XDG_CONFIG_HOME", &dir)
            .env("RUNG_CONFIG", dir.join("none.yaml"))
            .env("RUNG_HOME", &dir)
            .env("RUNG_BASE_URL", &url)
            .env("RUNG_MODEL", "m")
            .env("RUNG_API_KEY", "k")
            .env("RUNG_PROTOCOL", "openai")
            .env_remove("RUNG_TURN_CHECK")
            .env_remove("RUNG_KEY_FILE")
            .env_remove("RUNG_SYSTEM_PROMPT_FILE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut acp = Acp {
            child,
            stdin,
            stdout,
            dir,
            sid: String::new(),
            next_id: 1,
            bodies,
        };
        acp.ask("initialize", json!({"protocolVersion": 1}));
        let cwd = acp.dir.to_string_lossy().into_owned();
        let created = acp.ask("session/new", json!({"cwd": cwd, "mcpServers": []}));
        acp.sid = created.response["result"]["sessionId"]
            .as_str()
            .unwrap()
            .to_string();
        acp
    }

    fn ask(&mut self, method: &str, params: Value) -> Answer {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
        let mut updates = Vec::new();
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).expect("stdout line");
            let v: Value =
                serde_json::from_str(line.trim()).unwrap_or_else(|e| panic!("json {e}: {line}"));
            if v["id"] == id && v.get("method").is_none() {
                return Answer {
                    response: v,
                    updates,
                };
            }
            if v["method"] == "session/update" {
                updates.push(v["params"]["update"].clone());
            }
        }
    }

    fn prompt(&mut self, text: &str) -> Answer {
        let params = json!({"sessionId": self.sid, "prompt": [{"type": "text", "text": text}]});
        self.ask("session/prompt", params)
    }

    /// The next request body the model saw.
    fn body(&self) -> Value {
        self.bodies
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("a model request")
    }

    fn session(&self) -> Value {
        let file = self
            .dir
            .join(".rung/sessions")
            .join(format!("{}.json", self.sid));
        serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
    }
}

impl Drop for Acp {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

/// The `tool` messages a request carries, by call id.
fn tool_results(body: &Value) -> Vec<(String, String)> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "tool")
        .map(|m| {
            let content = match &m["content"] {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            (
                m["tool_call_id"].as_str().unwrap_or("").to_string(),
                content,
            )
        })
        .collect()
}

fn user_texts(body: &Value) -> Vec<String> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "user")
        .map(|m| match &m["content"] {
            Value::String(s) => s.clone(),
            // The current ask arrives as ACP prompt blocks.
            Value::Array(blocks) => blocks
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            other => other.to_string(),
        })
        .collect()
}

fn tool_call_ids(body: &Value) -> Vec<String> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "assistant")
        .flat_map(|m| m["tool_calls"].as_array().cloned().unwrap_or_default())
        .map(|c| c["id"].as_str().unwrap_or("").to_string())
        .collect()
}

/// Two earlier turns each read a large file; the third prompt overflows.
/// rung elides the oldest tool result, retries once and the turn ends
/// normally. The retry keeps every call and every user message. The elision
/// is in-memory only: the stored history is untouched, so the next prompt
/// overflows and elides again.
#[test]
fn an_overflowing_session_recovers_after_one_elision() {
    let dir = tempdir();
    big_file(&dir, "a.txt", "ALPHA");
    big_file(&dir, "b.txt", "BRAVO");
    let mut acp = Acp::start(
        dir,
        vec![
            read_call("call_a", "a.txt"),
            text_reply("read a"),
            read_call("call_b", "b.txt"),
            text_reply("read b"),
            overflow(),
            text_reply("recovered"),
            overflow(),
            text_reply("fourth"),
        ],
    );
    for (ask, n) in [("read a", 2), ("read b", 2)] {
        let a = acp.prompt(ask);
        assert_eq!(
            a.response["result"]["stopReason"], "end_turn",
            "{}",
            a.response
        );
        for _ in 0..n {
            acp.body();
        }
    }

    let third = acp.prompt("third");
    assert_eq!(
        third.response["result"]["stopReason"], "end_turn",
        "{}",
        third.response
    );
    let over = acp.body();
    let retry = acp.body();
    let before = tool_results(&over);
    assert!(before[0].1.contains("ALPHA-000"), "{over}");
    assert!(before[1].1.contains("BRAVO-000"), "{over}");

    let after = tool_results(&retry);
    assert_eq!(after.len(), 2, "{retry}");
    assert_eq!(after[0].0, "call_a");
    assert!(after[0].1.starts_with(ELIDED), "oldest not elided: {retry}");
    assert!(!retry.to_string().contains("ALPHA-000"), "{retry}");
    assert!(after[1].1.contains("BRAVO-000"), "newest elided: {retry}");
    assert_eq!(tool_call_ids(&retry), tool_call_ids(&over), "{retry}");
    assert_eq!(user_texts(&retry), user_texts(&over), "{retry}");
    assert_eq!(user_texts(&retry), ["read a", "read b", "third"], "{retry}");

    let session = acp.session().to_string();
    assert!(session.contains("recovered"), "{session}");
    assert!(session.contains("ALPHA-000"), "{session}");
    assert!(session.contains("BRAVO-000"), "{session}");

    let fourth = acp.prompt("fourth");
    assert_eq!(
        fourth.response["result"]["stopReason"], "end_turn",
        "{}",
        fourth.response
    );
    let over4 = acp.body();
    assert!(tool_results(&over4)[0].1.contains("ALPHA-000"), "{over4}");
    let retry4 = acp.body();
    let kept = tool_results(&retry4);
    assert_eq!(kept[0].0, "call_a");
    assert!(kept[0].1.starts_with(ELIDED), "{retry4}");
    assert!(kept[1].1.contains("BRAVO-000"), "{retry4}");
}

/// Still over the window after the one elision: the typed `overflow`
/// state, the provider's figures on a `usage_update`, nothing of the turn
/// stored, and the next prompt works.
#[test]
fn a_session_still_overflowing_after_one_elision_fails_typed_and_is_not_wedged() {
    let dir = tempdir();
    big_file(&dir, "a.txt", "ALPHA");
    let mut acp = Acp::start(
        dir,
        vec![
            read_call("call_a", "a.txt"),
            text_reply("read a"),
            overflow(),
            overflow(),
            text_reply("still here"),
        ],
    );
    let first = acp.prompt("read a");
    assert_eq!(first.response["result"]["stopReason"], "end_turn");
    acp.body();
    acp.body();
    let stored = acp.session();

    let second = acp.prompt("too much");
    let error = &second.response["error"];
    assert_eq!(error["code"], -32603, "{}", second.response);
    let terminal = &error["data"]["rung"]["terminal"];
    assert_eq!(terminal["state"], "overflow", "{}", second.response);
    assert!(
        terminal["reason"]
            .as_str()
            .unwrap()
            .starts_with("invalid-request (context-overflow): "),
        "{}",
        second.response
    );
    let usage = second
        .updates
        .iter()
        .rfind(|u| u["sessionUpdate"] == "usage_update")
        .unwrap_or_else(|| panic!("no usage_update: {:?}", second.updates));
    assert_eq!(usage["used"], 9100, "{usage}");
    assert_eq!(usage["size"], 8192, "{usage}");

    // Exactly one elision and one retry: two requests, the second elided.
    let over = acp.body();
    let retry = acp.body();
    assert!(tool_results(&over)[0].1.contains("ALPHA-000"), "{over}");
    assert!(tool_results(&retry)[0].1.starts_with(ELIDED), "{retry}");

    // The overflow turn left the session as it was.
    assert_eq!(acp.session()["lines"], stored["lines"]);

    let third = acp.prompt("next");
    assert_eq!(
        third.response["result"]["stopReason"], "end_turn",
        "wedged: {}",
        third.response
    );
    let next = acp.body();
    assert_eq!(user_texts(&next), ["read a", "next"], "{next}");
}

/// A turn whose tool calls ran and then overflowed, twice: nothing of it
/// is stored, not its ask and not its calls, and the next prompt does not
/// see it.
#[test]
fn an_overflow_turn_persists_nothing() {
    let dir = tempdir();
    big_file(&dir, "a.txt", "ALPHA");
    let mut acp = Acp::start(
        dir,
        vec![
            read_call("call_a", "a.txt"),
            overflow(),
            overflow(),
            text_reply("fresh"),
        ],
    );
    let first = acp.prompt("read a");
    let terminal = &first.response["error"]["data"]["rung"]["terminal"];
    assert_eq!(terminal["state"], "overflow", "{}", first.response);
    acp.body();
    acp.body();
    let retry = acp.body();
    // The elision reached this turn's own result: it was the only one.
    assert!(tool_results(&retry)[0].1.starts_with(ELIDED), "{retry}");

    let lines = acp.session()["lines"].clone();
    assert_eq!(lines, json!([]), "the overflow turn was stored: {lines}");

    let next = acp.prompt("again");
    assert_eq!(next.response["result"]["stopReason"], "end_turn");
    let body = acp.body();
    assert_eq!(user_texts(&body), ["again"], "{body}");
    assert!(tool_call_ids(&body).is_empty(), "{body}");
}
