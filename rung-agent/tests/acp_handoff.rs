//! `_rung/session/handoff`: the agent's own handoff text replaces every line
//! before the last N user turns. A session that never calls it is untouched.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-agent");

fn text_reply(text: &str) -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": text}, "finish_reason": "stop"}]})
}

struct Acp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Acp {
    fn start(cwd: &Path, url: &str) -> Self {
        let mut child = Command::new(BIN)
            .args(["--acp", "--tools", "read"])
            .current_dir(cwd)
            .env("HOME", cwd)
            .env("XDG_CONFIG_HOME", cwd)
            .env("RUNG_CONFIG", cwd.join("none.yaml"))
            .env("RUNG_HOME", cwd)
            .env("RUNG_BASE_URL", url)
            .env("RUNG_MODEL", "m")
            .env("RUNG_MODEL_FACTS", "off")
            .env("RUNG_API_KEY", "k")
            .env("RUNG_PROTOCOL", "openai")
            .env_remove("RUNG_TURN_CHECK")
            .env_remove("RUNG_KEY_FILE")
            .env_remove("RUNG_SYSTEM_PROMPT_FILE")
            .env_remove("RUNG_MEMORY")
            .env_remove("RUNG_REASONING")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
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

    fn ask(&mut self, sid: &str, text: &str) {
        let r = self.call(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": text}]}),
        );
        assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    }
}

impl Drop for Acp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn contents(body: &Value) -> Vec<String> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["content"].as_str().map(str::to_string))
        .collect()
}

/// Three answered turns, then (when `handoff`) a handoff keeping one turn,
/// then a fourth ask. Returns the fourth request's message contents, the
/// handoff's response, and the session file.
fn run(handoff: bool) -> (Vec<String>, Value, String) {
    let dir = rung_testkit::TempDir::new(if handoff { "handoff-on" } else { "handoff-off" });
    let (url, rx) = rung_testkit::llm::mock_llm(vec![
        text_reply("one"),
        text_reply("two"),
        text_reply("three"),
        text_reply("four"),
    ]);
    let mut acp = Acp::start(&dir, &url);
    let r = acp.call(
        "session/new",
        json!({"cwd": dir.to_string_lossy(), "mcpServers": []}),
    );
    let sid = r["result"]["sessionId"].as_str().unwrap().to_string();
    for t in ["first", "second", "third"] {
        acp.ask(&sid, t);
    }
    let mut resp = Value::Null;
    if handoff {
        resp = acp.call(
            "_rung/session/handoff",
            json!({"sessionId": sid, "handoff": "HANDOFF-TEXT: the key is kiwi-42", "keepLastTurns": 1}),
        );
    }
    acp.ask(&sid, "fourth");
    let mut last = Value::Null;
    while let Ok(b) = rx.recv_timeout(Duration::from_secs(20)) {
        last = b;
        if contents(&last).iter().any(|c| c == "fourth") {
            break;
        }
    }
    let file =
        std::fs::read_to_string(dir.join(".rung/sessions").join(format!("{sid}.jsonl"))).unwrap();
    (contents(&last), resp, file)
}

#[test]
fn a_handoff_replaces_everything_before_the_last_turns_with_one_summary() {
    let (msgs, resp, file) = run(true);
    let joined = msgs.join("\n");
    assert!(
        joined.contains("HANDOFF-TEXT: the key is kiwi-42"),
        "{msgs:?}"
    );
    assert!(!msgs.iter().any(|c| c == "first" || c == "one"), "{msgs:?}");
    assert!(
        !msgs.iter().any(|c| c == "second" || c == "two"),
        "{msgs:?}"
    );
    let pos = |s: &str| msgs.iter().position(|c| c == s).unwrap();
    assert!(
        pos("third") < pos("three") && pos("three") < pos("fourth"),
        "{msgs:?}"
    );
    let r = &resp["result"];
    assert_eq!(r["before"]["lines"], 6, "{resp}");
    assert_eq!(r["after"]["lines"], 3, "{resp}");
    assert!(r["after"]["tokens"].as_u64().unwrap() > 0, "{resp}");
    assert!(r["before"]["tokens"].as_u64().unwrap() > 0, "{resp}");
    assert!(
        file.contains("\"summary\""),
        "the log records the summary line"
    );
}

#[test]
fn without_a_handoff_the_session_is_untouched() {
    let (msgs, _, file) = run(false);
    for c in ["first", "one", "second", "two", "third", "three", "fourth"] {
        assert!(msgs.iter().any(|m| m == c), "{c} missing: {msgs:?}");
    }
    assert!(
        !file.contains("summary") && !file.contains("truncate"),
        "{file}"
    );
}

#[test]
fn a_handoff_that_folds_nothing_or_says_nothing_is_refused() {
    let dir = rung_testkit::TempDir::new("handoff-refused");
    let (url, _rx) = rung_testkit::llm::mock_llm(vec![text_reply("one")]);
    let mut acp = Acp::start(&dir, &url);
    let r = acp.call(
        "session/new",
        json!({"cwd": dir.to_string_lossy(), "mcpServers": []}),
    );
    let sid = r["result"]["sessionId"].as_str().unwrap().to_string();
    acp.ask(&sid, "first");
    let none = acp.call(
        "_rung/session/handoff",
        json!({"sessionId": sid, "handoff": "x", "keepLastTurns": 5}),
    );
    assert!(none.get("error").is_some(), "{none}");
    let empty = acp.call(
        "_rung/session/handoff",
        json!({"sessionId": sid, "handoff": "  ", "keepLastTurns": 0}),
    );
    assert!(empty.get("error").is_some(), "{empty}");
    let other = acp.call("_rung/session/other", json!({"sessionId": sid}));
    assert_eq!(other["error"]["code"], -32601, "{other}");
}
