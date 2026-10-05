//! A session loaded in a new `rung-agent --acp` process keeps the stable
//! part of its requests: the same system text (kept in the session file)
//! and the same tools (the MCP servers `session/load` and `session/resume`
//! name).
//! Without either, the next request shares no prefix with the last, and a
//! provider's prompt cache misses from the first byte.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-agent");

fn text_reply(text: &str) -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": text}, "finish_reason": "stop"}]})
}

/// `rung-agent --acp --tools read` in `cwd`, env isolated.
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

fn tool_names(body: &Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["function"]["name"].as_str().map(str::to_string))
        .collect()
}

/// `session/load` or `session/resume` in a new process: the next request
/// keeps the last one's tools and system text, byte for byte.
fn after_restart(method: &str) {
    let dir = rung_testkit::TempDir::new(&format!("acp-load-{}", method.replace('/', "-")));
    let notes = dir.join("notes.jsonl");
    let mcp = json!([{
        "name": "notes",
        "command": BIN,
        "args": ["--memory-fixture", "--file", notes.to_string_lossy()],
        "env": []
    }]);
    let (url, rx) = rung_testkit::llm::mock_llm(vec![text_reply("one"), text_reply("two")]);
    let recv = || rx.recv_timeout(Duration::from_secs(20)).expect("a request");

    let mut acp = Acp::start(&dir, &url);
    let r = acp.call(
        "session/new",
        json!({"cwd": dir.to_string_lossy(), "mcpServers": mcp, "_meta": {"systemPrompt": "Be brief."}}),
    );
    let sid = r["result"]["sessionId"].as_str().unwrap().to_string();
    acp.ask(&sid, "first");
    let r1 = recv();
    drop(acp);

    let mut acp = Acp::start(&dir, &url);
    let r = acp.call(
        method,
        json!({"sessionId": sid, "cwd": dir.to_string_lossy(), "mcpServers": mcp}),
    );
    assert!(r.get("result").is_some(), "{method}: {r}");
    acp.ask(&sid, "second");
    let r2 = recv();

    assert_eq!(
        r2["messages"][0],
        json!({"role": "system", "content": "Be brief."}),
        "{method}: the session's system text"
    );
    assert!(
        tool_names(&r2).contains(&"rung_memory_recall".to_string()),
        "{method}: the MCP server's tools: {:?}",
        tool_names(&r2)
    );
    assert_eq!(r1["tools"], r2["tools"], "{method}: the same tools");
    // The prefix through the system text is the same bytes: the tools,
    // then the system message.
    assert_eq!(
        r1["messages"][0].to_string(),
        r2["messages"][0].to_string(),
        "{method}: system bytes"
    );
}

#[test]
fn a_load_in_a_new_process_keeps_the_system_text_and_tools() {
    after_restart("session/load");
}

#[test]
fn a_resume_in_a_new_process_keeps_the_system_text_and_tools() {
    after_restart("session/resume");
}

#[test]
fn a_fork_in_a_new_process_keeps_the_system_text_and_tools() {
    let dir = rung_testkit::TempDir::new("acp-load-fork");
    let notes = dir.join("notes.jsonl");
    let mcp = json!([{
        "name": "notes",
        "command": BIN,
        "args": ["--memory-fixture", "--file", notes.to_string_lossy()],
        "env": []
    }]);
    let (url, rx) = rung_testkit::llm::mock_llm(vec![text_reply("one"), text_reply("two")]);
    let recv = || rx.recv_timeout(Duration::from_secs(20)).expect("a request");

    let mut acp = Acp::start(&dir, &url);
    let r = acp.call(
        "session/new",
        json!({"cwd": dir.to_string_lossy(), "mcpServers": mcp, "_meta": {"systemPrompt": "Be brief."}}),
    );
    let sid = r["result"]["sessionId"].as_str().unwrap().to_string();
    acp.ask(&sid, "first");
    let r1 = recv();
    drop(acp);

    let mut acp = Acp::start(&dir, &url);
    let r = acp.call(
        "session/fork",
        json!({"sessionId": sid, "cwd": dir.to_string_lossy(), "mcpServers": mcp}),
    );
    let child = r["result"]["sessionId"]
        .as_str()
        .expect("a forked id")
        .to_string();
    acp.ask(&child, "second");
    let r2 = recv();

    assert_eq!(r1["tools"], r2["tools"], "fork: the same tools");
    assert!(tool_names(&r2).contains(&"rung_memory_recall".to_string()));
    assert_eq!(
        r1["messages"][0].to_string(),
        r2["messages"][0].to_string(),
        "fork: system bytes"
    );
}
