//! The prompt-cache prefix over ACP: the same
//! session, turn after turn, against a mock model. A provider caches a
//! request's prefix, so each request must extend the one before it byte for
//! byte (cache markers aside: they mark, they are not content).
//!
//! - an all-text ask is sent in the form a later turn replays it in;
//! - a long tool result is replayed verbatim, not shortened;
//! - a recall block follows the ask and is replayed with it, so the prefix
//!   runs through the block and the steps after it;
//! - a marked context band sent every turn is in the history once.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-agent");

fn text_reply(text: &str) -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": text}, "finish_reason": "stop"}]})
}

fn read_call(id: &str, path: &str) -> Value {
    let args = json!({ "path": path }).to_string();
    json!({"id": "c", "model": "m", "choices": [{"message": {"tool_calls": [
        {"id": id, "type": "function", "function": {"name": "read_file", "arguments": args}}
    ]}, "finish_reason": "tool_calls"}]})
}

/// `rung-agent --acp` in `cwd`, env isolated.
struct Acp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Acp {
    fn start(cwd: &Path, url: &str, args: &[&str], env: &[(&str, &str)]) -> Self {
        let mut cmd = Command::new(BIN);
        cmd.arg("--acp")
            .args(args)
            .current_dir(cwd)
            .env("HOME", cwd)
            .env("XDG_CONFIG_HOME", cwd)
            .env("RUNG_CONFIG", cwd.join("none.yaml"))
            .env("RUNG_HOME", cwd)
            .env("RUNG_BASE_URL", url)
            .env("RUNG_MODEL", "m")
            .env("RUNG_API_KEY", "k")
            .env("RUNG_PROTOCOL", "openai")
            .env_remove("RUNG_TURN_CHECK")
            .env_remove("RUNG_KEY_FILE")
            .env_remove("RUNG_SYSTEM_PROMPT_FILE")
            .env_remove("RUNG_MEMORY")
            .env_remove("RUNG_REASONING")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut acp = Acp {
            child,
            stdin,
            stdout,
            next: 1,
        };
        acp.call("initialize", json!({"protocolVersion": 1}));
        acp
    }

    /// Send a request and return its response, skipping notifications.
    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).unwrap() > 0,
                "agent exited"
            );
            let v: Value = serde_json::from_str(line.trim()).unwrap();
            if v["id"] == id && v.get("method").is_none() {
                return v;
            }
        }
    }

    fn new_session(&mut self, cwd: &Path, mcp: Value, system: &str) -> String {
        let r = self.call(
            "session/new",
            json!({"cwd": cwd.to_string_lossy(), "mcpServers": mcp, "_meta": {"systemPrompt": system}}),
        );
        r["result"]["sessionId"].as_str().unwrap().to_string()
    }

    fn prompt(&mut self, sid: &str, blocks: Value) {
        let r = self.call(
            "session/prompt",
            json!({"sessionId": sid, "prompt": blocks}),
        );
        assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    }

    fn ask(&mut self, sid: &str, text: &str) {
        self.prompt(sid, json!([{"type": "text", "text": text}]));
    }
}

impl Drop for Acp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn recv(rx: &Receiver<Value>) -> Value {
    rx.recv_timeout(Duration::from_secs(20))
        .expect("a model request")
}

/// The value with every `cache_control` removed.
fn unmarked(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| k.as_str() != "cache_control")
                .map(|(k, x)| (k.clone(), unmarked(x)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(unmarked).collect()),
        other => other.clone(),
    }
}

/// `next` extends `prev`: every field but the messages is the same, and
/// `prev`'s messages are the first of `next`'s, byte for byte.
fn assert_extends(prev: &Value, next: &Value) {
    let (prev, next) = (unmarked(prev), unmarked(next));
    let fields = |v: &Value| {
        let mut o = v.as_object().unwrap().clone();
        o.remove("messages");
        Value::Object(o)
    };
    assert_eq!(fields(&prev), fields(&next), "tools and settings");
    let (a, b) = (
        prev["messages"].as_array().unwrap(),
        next["messages"].as_array().unwrap(),
    );
    assert!(a.len() < b.len(), "{} then {} messages", a.len(), b.len());
    for (i, m) in a.iter().enumerate() {
        assert_eq!(m.to_string(), b[i].to_string(), "message {i} changed");
    }
}

fn messages(v: &Value) -> &Vec<Value> {
    v["messages"].as_array().unwrap()
}

#[test]
fn an_all_text_ask_is_sent_as_the_bytes_a_later_turn_replays() {
    let dir = rung_testkit::TempDir::new("acp-prefix-ask");
    let (url, rx) = rung_testkit::llm::mock_llm(vec![
        text_reply("one"),
        text_reply("two"),
        text_reply("three"),
    ]);
    let mut acp = Acp::start(&dir, &url, &["--tools", "none"], &[]);
    let sid = acp.new_session(&dir, json!([]), "Be brief.");
    acp.ask(&sid, "first ask");
    let r1 = recv(&rx);
    // Two text blocks: sent joined, as the session line holds them.
    acp.prompt(
        &sid,
        json!([{"type": "text", "text": "second ask"}, {"type": "text", "text": "in one word"}]),
    );
    let r2 = recv(&rx);
    acp.ask(&sid, "third ask");
    let r3 = recv(&rx);
    assert_eq!(
        messages(&r1)[1],
        json!({"role": "user", "content": "first ask"})
    );
    assert_eq!(
        messages(&r2)[3],
        json!({"role": "user", "content": "second ask\nin one word"})
    );
    assert_extends(&r1, &r2);
    assert_extends(&r2, &r3);
    assert!(r3.get("session_id").is_none(), "a plain route gets no id");
}

#[test]
fn a_long_tool_result_is_replayed_verbatim() {
    let dir = rung_testkit::TempDir::new("acp-prefix-tool");
    let text: String = (0..150)
        .map(|i| format!("line-{i:03} lorem ipsum dolor sit amet\n"))
        .collect();
    assert!(text.len() > 4000);
    std::fs::write(dir.join("big.txt"), &text).unwrap();
    let (url, rx) = rung_testkit::llm::mock_llm(vec![
        read_call("call_1", "big.txt"),
        text_reply("read it"),
        text_reply("next"),
    ]);
    let mut acp = Acp::start(&dir, &url, &["--tools", "read"], &[]);
    let sid = acp.new_session(&dir, json!([]), "Be brief.");
    acp.ask(&sid, "read big.txt");
    let _ = recv(&rx);
    let r2 = recv(&rx);
    acp.ask(&sid, "and now?");
    let r3 = recv(&rx);
    let result = messages(&r3)[3]["content"].as_str().unwrap();
    assert!(result.contains("line-149"), "the whole result: {result}");
    assert_eq!(result, messages(&r2)[3]["content"].as_str().unwrap());
    assert!(!result.contains("shortened"));
    assert_extends(&r2, &r3);
}

#[test]
fn a_recall_block_follows_the_ask_and_is_replayed_with_it() {
    let dir = rung_testkit::TempDir::new("acp-prefix-recall");
    let (url, rx) = rung_testkit::llm::mock_llm(vec![
        text_reply("Noted."),
        text_reply("release/x"),
        text_reply("ok"),
    ]);
    let args = ["--tools", "none"];
    let mut acp = Acp::start(&dir, &url, &args, &[("RUNG_MEMORY", "baseline")]);
    let sid = acp.new_session(&dir, json!([]), "Be brief.");
    acp.ask(&sid, "Remember: the deploy branch is release/x");
    let _ = recv(&rx);
    acp.ask(&sid, "Which deploy branch?");
    let r2 = recv(&rx);
    acp.ask(&sid, "thanks");
    let r3 = recv(&rx);
    let sent = messages(&r2)[3]["content"].as_str().unwrap();
    let (ask, block) = sent.split_once("\n\n---\n").expect("a recall block");
    assert_eq!(ask, "Which deploy branch?");
    assert!(block.starts_with("## Recalled memory"), "{block}");
    // The next turn replays the ask with its block, byte for byte.
    assert_eq!(messages(&r3)[3], json!({"role": "user", "content": sent}));
    assert_extends(&r2, &r3);
}

/// Memory on, a marked context band in front of every ask, and a tool step
/// after the recall: every request extends the one before, so a turn reads
/// only its own new messages uncached.
#[test]
fn with_memory_and_a_context_band_each_request_extends_the_last() {
    let dir = rung_testkit::TempDir::new("acp-prefix-band");
    std::fs::write(dir.join("notes.txt"), "a\nb\nc\n").unwrap();
    let (url, rx) = rung_testkit::llm::mock_llm(vec![
        text_reply("Noted."),
        read_call("call_1", "notes.txt"),
        text_reply("3 lines; release/x"),
        text_reply("release/x"),
    ]);
    let args = ["--tools", "read"];
    let mut acp = Acp::start(&dir, &url, &args, &[("RUNG_MEMORY", "baseline")]);
    let sid = acp.new_session(&dir, json!([]), "Be brief.");
    let band = json!({"type": "text", "text": "orientation: repo rung",
                      "annotations": {"audience": ["assistant"]}});
    let ask = |t: &str| json!([band, {"type": "text", "text": t}]);
    acp.prompt(&sid, ask("Remember: the deploy branch is release/x"));
    let r1 = recv(&rx);
    acp.prompt(
        &sid,
        ask("Count the lines of notes.txt; which deploy branch?"),
    );
    let (r2, r2b) = (recv(&rx), recv(&rx));
    acp.prompt(&sid, ask("Which deploy branch, again?"));
    let r3 = recv(&rx);
    let recalled = messages(&r2)[3]["content"].as_str().unwrap();
    assert!(
        recalled.contains("\n\n---\n## Recalled memory"),
        "{recalled}"
    );
    assert_extends(&r1, &r2);
    assert_extends(&r2, &r2b);
    assert_extends(&r2b, &r3);
    // The band is in the history once, in the first ask that sent it.
    let banded = messages(&r3)
        .iter()
        .filter(|m| m.to_string().contains("orientation: repo rung"))
        .count();
    assert_eq!(banded, 1, "{r3}");
}
