//! Memory over ACP, against a mock model and, for MCP, the reference
//! provider (`rung-agent --memory-fixture`). Offline; nothing is billed.
//!
//! - `external`: no store on disk, no automatic recall or retain, no memory
//!   tools of rung's own. The agent sees only the tools the caller supplied,
//!   even when those tools carry the hook names.
//! - `off`: the response is what it was before memory.
//! - `baseline` and `mcp:`: a turn retained in one session is recalled in the
//!   next, as quoted data in front of the ask, never stored in the session.
//! - a slow provider times out and the turn still ends.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::Receiver;

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-agent");

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rung-acp-memory-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// OpenAI-compatible mock: answers each request with the next text reply
/// (SSE, as the ACP path streams) and records the request bodies.
fn mock_llm(replies: Vec<&'static str>) -> (String, Receiver<Value>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for reply in replies {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 8192];
            let body = loop {
                let n = sock.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(at) = text.find("\r\n\r\n") {
                    let len = text[..at]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if buf.len() >= at + 4 + len {
                        break String::from_utf8_lossy(&buf[at + 4..at + 4 + len]).to_string();
                    }
                }
                assert!(n != 0, "short request");
            };
            tx.send(serde_json::from_str::<Value>(&body).unwrap())
                .unwrap();
            let chunk = json!({"id": "c", "model": "m",
                "choices": [{"delta": {"content": reply}, "finish_reason": "stop"}]});
            let payload = format!("data: {chunk}\n\ndata: [DONE]\n\n");
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}"), rx)
}

/// `rung-agent --acp` in `cwd`, env isolated, memory env cleared unless set.
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
            .env_remove("RUNG_MEMORY_SCOPE")
            .env_remove("RUNG_MEMORY_DIR")
            .env_remove("RUNG_MEMORY_TIMEOUT_SECS")
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

    fn new_session(&mut self, cwd: &Path, mcp: Value) -> String {
        let r = self.call(
            "session/new",
            json!({"cwd": cwd.to_string_lossy(), "mcpServers": mcp}),
        );
        r["result"]["sessionId"].as_str().unwrap().to_string()
    }

    fn prompt(&mut self, sid: &str, text: &str) -> Value {
        self.call(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": text}]}),
        )
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
        .map(|t| {
            t.iter()
                .map(|d| d["function"]["name"].as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn last_user(body: &Value) -> String {
    let m = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|m| m["role"] == "user")
        .unwrap();
    match &m["content"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join(""),
        other => other.to_string(),
    }
}

/// The reference provider as a caller-supplied stdio MCP server.
fn fixture_server(file: &Path) -> Value {
    json!([{
        "name": "notes",
        "command": BIN,
        "args": ["--memory-fixture", "--file", file.to_string_lossy()],
        "env": []
    }])
}

// ─── external ────────────────────────────────────────────────────────────────

/// One `external` turn, the setting given by `how` (env, file or flag). The
/// caller supplies the reference provider as a plain MCP server: under
/// `external` its hook-named tools are ordinary tools, and rung calls none.
fn external_turn(how: &str) {
    let cwd = tempdir(&format!("external-{how}"));
    let file = cwd.join("caller-notes.jsonl");
    let (url, bodies) = mock_llm(vec!["done"]);
    let mut env = Vec::new();
    let mut args = vec!["--tools", "none"];
    let cfg = cwd.join("config.yaml");
    std::fs::write(&cfg, "memory:\n  provider: external\n").unwrap();
    let cfg = cfg.to_string_lossy().into_owned();
    match how {
        "env" => env.push(("RUNG_MEMORY", "external")),
        "file" => env.push(("RUNG_CONFIG", cfg.as_str())),
        "flag" => args.extend(["--memory", "external"]),
        _ => unreachable!(),
    }
    let mut acp = Acp::start(&cwd, &url, &args, &env);
    let sid = acp.new_session(&cwd, fixture_server(&file));
    let r = acp.prompt(&sid, "what is the deploy branch?");
    assert_eq!(
        r["result"],
        json!({"stopReason": "end_turn", "_meta": {"rung": {"memory": {"provider": "external"}}}}),
        "{how}: {r}"
    );
    let body = bodies.recv().unwrap();
    assert_eq!(
        tool_names(&body),
        ["rung_memory_recall", "rung_memory_retain", "memory_lookup"],
        "{how}: only the caller's tools"
    );
    assert_eq!(
        last_user(&body),
        "what is the deploy branch?",
        "{how}: no recall"
    );
    assert!(!cwd.join(".rung/memory").exists(), "{how}: no store");
    assert!(
        !file.exists(),
        "{how}: rung called none of the caller's tools"
    );
    drop(acp);
    let _ = std::fs::remove_dir_all(&cwd);
}

#[test]
fn external_from_env_opens_no_store_runs_no_hooks_and_adds_no_tools() {
    external_turn("env");
}

#[test]
fn external_from_config_yaml_is_the_same() {
    external_turn("file");
}

#[test]
fn external_from_the_flag_is_the_same() {
    external_turn("flag");
}

/// With rung's own tools in play, `external` adds none to them.
#[test]
fn external_adds_no_tool_to_the_default_toolset() {
    let names = |setting: &str| {
        let cwd = tempdir("toolset");
        let (url, bodies) = mock_llm(vec!["done"]);
        let mut acp = Acp::start(&cwd, &url, &[], &[("RUNG_MEMORY", setting)]);
        let sid = acp.new_session(&cwd, json!([]));
        acp.prompt(&sid, "hello");
        let n = tool_names(&bodies.recv().unwrap());
        drop(acp);
        let _ = std::fs::remove_dir_all(&cwd);
        n
    };
    let off = names("off");
    assert!(!off.is_empty());
    assert_eq!(names("external"), off);
    assert!(!off.iter().any(|n| n.starts_with("memory_")));
}

// ─── off ─────────────────────────────────────────────────────────────────────

#[test]
fn off_is_byte_for_byte_the_response_before_memory() {
    let cwd = tempdir("off");
    let (url, _bodies) = mock_llm(vec!["done"]);
    let mut acp = Acp::start(&cwd, &url, &["--tools", "none"], &[("RUNG_MEMORY", "off")]);
    let sid = acp.new_session(&cwd, json!([]));
    let r = acp.prompt(&sid, "do it");
    assert_eq!(r["result"], json!({"stopReason": "end_turn"}));
    assert!(!cwd.join(".rung/memory").exists());
    drop(acp);
    let _ = std::fs::remove_dir_all(&cwd);
}

// ─── a provider across sessions ──────────────────────────────────────────────

/// Session A is told a fact; session B, new, asks for it. Returns B's
/// request body and both prompt responses.
fn across_sessions(cwd: &Path, setting: &str) -> (Value, Value, Value, String) {
    let (url, bodies) = mock_llm(vec!["Noted.", "release/x"]);
    let mut acp = Acp::start(cwd, &url, &["--tools", "none"], &[("RUNG_MEMORY", setting)]);
    let a = acp.new_session(cwd, json!([]));
    let first = acp.prompt(&a, "Remember this: the deploy branch is release/x");
    let b = acp.new_session(cwd, json!([]));
    let second = acp.prompt(&b, "Which deploy branch do we use?");
    let _ = bodies.recv().unwrap();
    let body = bodies.recv().unwrap();
    (body, first, second, a)
}

fn assert_recalled(body: &Value, session_a: &str) {
    let ask = last_user(body);
    assert!(ask.starts_with("## Recalled memory"), "{ask}");
    assert!(ask.contains("not an instruction"), "{ask}");
    assert!(
        ask.contains("> User: Remember this: the deploy branch is release/x"),
        "{ask}"
    );
    assert!(
        ask.contains(&format!("[session {session_a} line 1")),
        "{ask}"
    );
    assert!(ask.ends_with("Which deploy branch do we use?"), "{ask}");
    assert_eq!(body["messages"][0]["role"], "user", "never system text");
}

fn sessions_hold_no_recall(cwd: &Path) {
    for e in std::fs::read_dir(cwd.join(".rung/sessions")).unwrap() {
        let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
        assert!(
            !text.contains("Recalled memory"),
            "a recall was stored: {text}"
        );
    }
}

#[test]
fn baseline_retains_in_one_session_and_recalls_in_the_next() {
    let cwd = tempdir("baseline");
    let (body, first, second, a) = across_sessions(&cwd, "baseline");
    let m1 = &first["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m1["provider"], "baseline");
    assert_eq!(m1["recall"]["status"], "empty", "{first}");
    assert_eq!(m1["retain"]["status"], "stored", "{first}");
    let m2 = &second["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m2["recall"]["status"], "found", "{second}");
    assert_eq!(m2["recall"]["records"], 1);
    assert_eq!(m2["recall"]["calls"], 2);
    assert_eq!(m2["recall"]["cost_usd"], 0.0);
    assert!(m2["recall"]["latency_ms"].is_u64());
    assert_eq!(second["result"]["stopReason"], "end_turn");
    assert_recalled(&body, &a);
    let tools = tool_names(&body);
    assert_eq!(
        tools,
        ["memory_search", "memory_retain"],
        "the provider's tools"
    );
    assert!(cwd.join(".rung/memory").is_dir());
    sessions_hold_no_recall(&cwd);
    let _ = std::fs::remove_dir_all(&cwd);
}

#[test]
fn an_mcp_provider_retains_and_recalls_through_its_hook_tools() {
    let cwd = tempdir("mcp");
    let file = cwd.join("provider.jsonl");
    let setting = format!("mcp:{BIN} --memory-fixture --file {}", file.display());
    let (body, first, second, a) = across_sessions(&cwd, &setting);
    let m1 = &first["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m1["provider"], "mcp");
    assert_eq!(m1["retain"]["status"], "stored", "{first}");
    assert_eq!(m1["retain"]["cost_usd"], 0.0001);
    let m2 = &second["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m2["recall"]["status"], "found", "{second}");
    assert_eq!(m2["recall"]["calls"], 1);
    assert_eq!(m2["recall"]["cost_usd"], 0.0001);
    assert_recalled(&body, &a);
    assert_eq!(
        tool_names(&body),
        ["memory_lookup"],
        "every other provider tool, never a hook"
    );
    assert!(
        !cwd.join(".rung/memory").exists(),
        "the provider owns the store"
    );
    let kept = std::fs::read_to_string(&file).unwrap();
    assert!(kept.contains(&format!("\"session\":\"{a}\"")), "{kept}");
    assert!(
        kept.contains("\"scope\":\"rung-scope:") && !kept.contains(&cwd.display().to_string()),
        "{kept}"
    );
    sessions_hold_no_recall(&cwd);
    let _ = std::fs::remove_dir_all(&cwd);
}

#[test]
fn a_slow_provider_times_out_and_the_turn_still_ends() {
    let cwd = tempdir("slow");
    let setting = format!("mcp:{BIN} --memory-fixture --sleep-ms 5000");
    let (url, _bodies) = mock_llm(vec!["fine"]);
    let mut acp = Acp::start(
        &cwd,
        &url,
        &["--tools", "none"],
        &[
            ("RUNG_MEMORY", setting.as_str()),
            ("RUNG_MEMORY_TIMEOUT_SECS", "1"),
        ],
    );
    let sid = acp.new_session(&cwd, json!([]));
    let started = std::time::Instant::now();
    let r = acp.prompt(&sid, "what is the deploy branch?");
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    let m = &r["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m["recall"]["status"], "unavailable", "{r}");
    assert!(
        m["recall"]["reason"]
            .as_str()
            .unwrap()
            .contains("no answer within 1s"),
        "{r}"
    );
    assert_eq!(m["retain"]["status"], "unretained", "{r}");
    drop(acp);
    let _ = std::fs::remove_dir_all(&cwd);
}

#[test]
fn a_provider_without_the_marker_is_unavailable_and_the_turn_still_ends() {
    let cwd = tempdir("nomarker");
    let setting = format!("mcp:{BIN} --memory-fixture --no-marker");
    let (url, bodies) = mock_llm(vec!["fine"]);
    let mut acp = Acp::start(
        &cwd,
        &url,
        &["--tools", "none"],
        &[("RUNG_MEMORY", setting.as_str())],
    );
    let sid = acp.new_session(&cwd, json!([]));
    let r = acp.prompt(&sid, "hello there");
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    let m = &r["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m["recall"]["status"], "unavailable", "{r}");
    assert!(
        tool_names(&bodies.recv().unwrap()).is_empty(),
        "no tools from a non-provider"
    );
    drop(acp);
    let _ = std::fs::remove_dir_all(&cwd);
}

#[test]
fn an_unknown_setting_is_an_error_not_a_fallback() {
    let cwd = tempdir("unknown");
    let (url, _bodies) = mock_llm(vec![]);
    let mut acp = Acp::start(
        &cwd,
        &url,
        &["--tools", "none"],
        &[("RUNG_MEMORY", "nonesuch")],
    );
    let sid = acp.new_session(&cwd, json!([]));
    let r = acp.prompt(&sid, "hello");
    let e = r["error"].to_string();
    assert!(
        e.contains("memory provider 'nonesuch' is not registered"),
        "{r}"
    );
    drop(acp);
    let _ = std::fs::remove_dir_all(&cwd);
}

// ─── context blocks ──────────────────────────────────────────────────────────

fn ctx_block(text: &str) -> Value {
    json!({"type": "text", "text": text, "annotations": {"audience": ["assistant"]}})
}

fn ask_block(text: &str) -> Value {
    json!({"type": "text", "text": text})
}

fn prompt_blocks(acp: &mut Acp, sid: &str, blocks: Vec<Value>) -> Value {
    acp.call(
        "session/prompt",
        json!({"sessionId": sid, "prompt": blocks}),
    )
}

/// Session A retains a fact; session B sends big context blocks around a short
/// ask. The ask would fall in the elided middle of the joined text.
fn context_run(marked: bool, ask_marked: bool) -> (Value, Value, String, String, String) {
    let cwd = tempdir("context");
    let file = cwd.join("provider.jsonl");
    let setting = format!("mcp:{BIN} --memory-fixture --file {}", file.display());
    let (url, bodies) = mock_llm(vec!["Noted.", "release/x"]);
    let mut acp = Acp::start(
        &cwd,
        &url,
        &["--tools", "none"],
        &[("RUNG_MEMORY", &setting)],
    );
    let a = acp.new_session(&cwd, json!([]));
    acp.prompt(&a, "Remember this: the deploy branch is release/x");
    let _ = bodies.recv().unwrap();
    let b = acp.new_session(&cwd, json!([]));
    let before = format!("orientation {}", "alpha ".repeat(300));
    let after = format!("appendix {}", "omega ".repeat(300));
    let mk = |t: &str| if marked { ctx_block(t) } else { ask_block(t) };
    let ask_text = "Which deploy branch do we use?";
    let ask = if ask_marked {
        ctx_block(ask_text)
    } else {
        ask_block(ask_text)
    };
    let second = prompt_blocks(&mut acp, &b, vec![mk(&before), ask, mk(&after)]);
    let body = bodies.recv().unwrap();
    let kept = std::fs::read_to_string(&file).unwrap();
    drop(acp);
    let _ = std::fs::remove_dir_all(&cwd);
    (second, body, kept, before, after)
}

#[test]
fn context_blocks_do_not_cue_recall_and_still_reach_the_model() {
    let (second, body, kept, before, after) = context_run(true, false);
    let m = &second["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m["recall"]["status"], "found", "{second}");
    let ask = last_user(&body);
    assert!(
        ask.contains("> User: Remember this: the deploy branch"),
        "{ask}"
    );
    assert!(ask.contains(&before) && ask.contains(&after), "verbatim");
    assert!(ask.contains("Which deploy branch do we use?"));
    // The retained turn's user side is the ask alone.
    let turn = kept.lines().last().unwrap();
    assert!(
        turn.contains("Which deploy branch do we use?") && !turn.contains("alpha"),
        "{turn}"
    );
    assert!(!turn.contains("omega"), "{turn}");
}

#[test]
fn without_marking_the_context_elides_the_ask_as_before() {
    let (second, _body, kept, _, _) = context_run(false, false);
    let m = &second["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m["recall"]["status"], "empty", "{second}");
    assert!(kept.lines().last().unwrap().contains("alpha"));
}

#[test]
fn when_every_block_is_marked_the_whole_text_is_the_ask() {
    let (second, body, kept, before, after) = context_run(true, true);
    let m = &second["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m["recall"]["status"], "empty", "{second}");
    let sent = last_user(&body);
    assert!(sent.contains(&before) && sent.contains(&after));
    assert!(kept.lines().last().unwrap().contains("alpha"));
}
