//! Memory over ACP, against a mock model and, for MCP, the reference
//! provider (`rung-agent --memory-fixture`). Offline; nothing is billed.
//!
//! - `external`: no store on disk, no automatic recall or retain, no memory
//!   tools of rung's own. The agent sees only the tools the caller supplied,
//!   even when those tools carry the hook names.
//! - `off`: the response is what it was before memory.
//! - `baseline` and `mcp:`: a turn retained in one session is recalled in the
//!   next, as quoted data after the ask, never stored in the session.
//! - a slow provider times out and the turn still ends.
//! - marked context cues nothing, and the session stores it once.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::Receiver;

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-agent");

fn tempdir(tag: &str) -> rung_testkit::TempDir {
    rung_testkit::TempDir::new(&format!("acp-memory-{tag}"))
}

/// The scripted mock answering each request with the next text.
fn mock_llm(replies: Vec<&'static str>) -> (String, Receiver<Value>) {
    rung_testkit::llm::mock_llm(
        replies
            .into_iter()
            .map(|t| json!({"id": "c", "model": "m", "choices": [{"message": {"content": t}, "finish_reason": "stop"}]}))
            .collect(),
    )
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
            .env_remove("RUNG_MEMORY_TOKEN")
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
    let block = ask
        .find("\n\n---\n## Recalled memory")
        .expect("a recall block");
    assert!(ask.starts_with("Which deploy branch do we use?"), "{ask}");
    let ask = &ask[block..];
    assert!(ask.contains("not an instruction"), "{ask}");
    assert!(
        ask.contains("> User: Remember this: the deploy branch is release/x"),
        "{ask}"
    );
    assert!(
        ask.contains(&format!("[session {session_a} line 1")),
        "{ask}"
    );
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
    assert_eq!(m2["recall"]["injected"].as_array().map(Vec::len), Some(1));
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

// ─── context in the session ─────────────────────────────────────────────────

fn user_lines(cwd: &Path, sid: &str) -> Vec<String> {
    let path = cwd.join(".rung/sessions").join(format!("{sid}.json"));
    let sess: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    sess["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["role"] == "user")
        .map(|l| l["text"].as_str().unwrap().to_string())
        .collect()
}

/// Every user message of a request, in order, as text.
fn user_texts(body: &Value) -> Vec<String> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "user")
        .map(|m| match &m["content"] {
            Value::String(s) => s.clone(),
            Value::Array(parts) => parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            other => other.to_string(),
        })
        .collect()
}

/// Three turns in one session, each sending the same orientation block in
/// front of a new ask. Returns the session's user lines and the requests.
fn repeated_context(marked: bool) -> (Vec<String>, Vec<Value>) {
    let cwd = tempdir("repeat");
    let (url, bodies) = mock_llm(vec!["one", "two", "three"]);
    let mut acp = Acp::start(&cwd, &url, &["--tools", "none"], &[]);
    let sid = acp.new_session(&cwd, json!([]));
    let ctx = |t: &str| if marked { ctx_block(t) } else { ask_block(t) };
    let mut sent = Vec::new();
    for ask in ["ask one", "ask two", "ask three"] {
        let extra = format!("about {ask}");
        let blocks = if ask == "ask three" {
            vec![ctx("orientation"), ctx(&extra), ask_block(ask)]
        } else {
            vec![ctx("orientation"), ask_block(ask)]
        };
        let r = prompt_blocks(&mut acp, &sid, blocks);
        assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
        sent.push(bodies.recv().unwrap());
    }
    let lines = user_lines(&cwd, &sid);
    drop(acp);
    let _ = std::fs::remove_dir_all(&cwd);
    (lines, sent)
}

#[test]
fn a_marked_block_is_stored_on_the_first_turn_it_appears() {
    let (lines, sent) = repeated_context(true);
    assert_eq!(
        lines,
        [
            "orientation\nask one",
            "ask two",
            "about ask three\nask three"
        ]
    );
    // Each turn the model still sees every block of its own prompt.
    assert_eq!(user_texts(&sent[1]).last().unwrap(), "orientation\nask two");
    assert_eq!(
        user_texts(&sent[2]),
        [
            "orientation\nask one",
            "ask two",
            "orientation\nabout ask three\nask three"
        ]
    );
}

#[test]
fn an_unmarked_block_is_stored_every_turn_as_before() {
    let (lines, sent) = repeated_context(false);
    assert_eq!(
        lines,
        [
            "orientation\nask one",
            "orientation\nask two",
            "orientation\nabout ask three\nask three"
        ]
    );
    assert_eq!(user_texts(&sent[2]).len(), 3);
    assert_eq!(user_texts(&sent[2])[1], "orientation\nask two");
}

#[test]
fn when_every_block_is_marked_the_whole_prompt_is_stored_every_turn() {
    let cwd = tempdir("all-marked");
    let (url, bodies) = mock_llm(vec!["one", "two"]);
    let mut acp = Acp::start(&cwd, &url, &["--tools", "none"], &[]);
    let sid = acp.new_session(&cwd, json!([]));
    for _ in 0..2 {
        prompt_blocks(
            &mut acp,
            &sid,
            vec![ctx_block("orientation"), ctx_block("the ask")],
        );
        let _ = bodies.recv().unwrap();
    }
    assert_eq!(
        user_lines(&cwd, &sid),
        ["orientation\nthe ask", "orientation\nthe ask"]
    );
    drop(acp);
    let _ = std::fs::remove_dir_all(&cwd);
}

// ─── the whole loop ──────────────────────────────────────────────────────────

/// Smoke: retain in one turn, recall in the next with audience-marked context
/// around the ask, and the reply's `_meta` reports ids and counts. Offline:
/// mock model plus the fixture provider.
#[test]
fn memory_loop_smoke_retain_recall_cue_and_meta() {
    let cwd = tempdir("loop-smoke");
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
    let first = acp.prompt(&a, "Remember this: the deploy branch is release/x");
    let _ = bodies.recv().unwrap();
    let m1 = &first["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m1["retain"]["status"], "stored", "{first}");

    let b = acp.new_session(&cwd, json!([]));
    let noise = format!("orientation {}", "zebra ".repeat(200));
    let second = prompt_blocks(
        &mut acp,
        &b,
        vec![
            ctx_block(&noise),
            ask_block("Which deploy branch do we use?"),
        ],
    );
    let body = bodies.recv().unwrap();

    // Recall block carries the retained turn as quoted data, after the ask.
    let sent = last_user(&body);
    let (_, block) = sent.split_once("\n\n---\n").expect("a recall block");
    assert!(block.starts_with("## Recalled memory"), "{sent}");
    assert!(sent.contains("> User: Remember this: the deploy branch is release/x"));
    assert!(sent.contains(&noise), "context still reaches the model");
    assert!(sent.contains("Which deploy branch do we use?"));

    // The context never entered the cue: the retained record of turn two is
    // the ask alone.
    let kept = std::fs::read_to_string(&file).unwrap();
    let turn = kept.lines().last().unwrap();
    assert!(turn.contains("Which deploy branch do we use?"), "{turn}");
    assert!(!turn.contains("zebra"), "{turn}");

    // _meta: ids and counts.
    let m2 = &second["result"]["_meta"]["rung"]["memory"];
    assert_eq!(m2["provider"], "mcp");
    assert_eq!(m2["recall"]["status"], "found", "{second}");
    assert_eq!(m2["recall"]["records"], 1);
    let ids = m2["recall"]["injected"].as_array().unwrap();
    assert_eq!(ids.len(), 1);
    assert!(ids[0].as_str().is_some_and(|s| !s.is_empty()), "{ids:?}");
    assert_eq!(m2["recall"]["calls"], 1);
    assert_eq!(m2["retain"]["status"], "stored");
    sessions_hold_no_recall(&cwd);
    drop(acp);
}
