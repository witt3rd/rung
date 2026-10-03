//! The TurnCheck rung, end to end and in process (Jev plan Phase 1).
//!
//! Nothing here touches the network. The agent model is a local mock that
//! replays canned replies; the judge is a local mock of `POST /systemone`
//! whose answers come from [`Recorded`] fixtures under
//! `tests/fixtures/decide/turn_check/`. The real binary talks to the mock
//! through the real `JevDecider`.
//!
//! Fixtures were recorded from live Jev (`typesafe/jev-1.13`) with:
//!
//! ```text
//! RUNG_DECIDE=record doppler run -p fleet -c dev_work -- \
//!   cargo test -p rung-agent --test turn_check -- --test-threads=1
//! ```
//!
//! A test that drives the binary names the fixture each judge call replays.
//! Paths under the test's temp dir are written `<tmp>` in the recorded
//! request, so a fixture does not depend on where the test ran.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use rung_agent::turn_check::{self, Turn, TurnReading, arm, turn_ask, turncheck};
use rung_std::agent::AgentResult;
use rung_std::decide::{Ask, Decided, Decider, Recorded, Undecided, read_answers};
use rung_std::llm::{ChatMessage, MessageContent, MessageContentBlock, Usage};
use serde_json::{Value, json};

// ─── Fixtures and mocks ──────────────────────────────────────────────────────

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/decide/turn_check")
        .join(format!("{name}.json"))
}

fn transcript(name: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/transcripts")
        .join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

const NARRATE_REQUEST: &str = "Keep Mohak's test (whether we can think together without every idea turning into a task) as a note in notes.txt.";
/// The #128 narration, as spire-venue saw it: the reply claims the note.
const NARRATION: &str = "I kept Mohak's test as a note in notes.txt.";
/// A real answer from a rung-agent session (animus, 2026-08).
const INFO_REQUEST: &str = "In one sentence: who built Animus?";
const INFO_ANSWER: &str = "Donald built Animus — the wizard who built the substrate before I existed and has been tending it ever since. 🧙\u{200d}♂️";

fn text_reply(text: &str) -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": text}, "finish_reason": "stop"}]})
}

fn write_note_reply() -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": null, "tool_calls": [
        {"id": "c1", "type": "function", "function": {"name": "write_file",
         "arguments": "{\"path\": \"notes.txt\", \"content\": \"Mohak's test\\n\"}"}}
    ]}, "finish_reason": "tool_calls"}]})
}

/// Read one HTTP request; return its body.
fn read_body(sock: &mut std::net::TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = sock.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
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
                return Some(String::from_utf8_lossy(&buf[at + 4..at + 4 + len]).to_string());
            }
        }
    }
}

fn respond(sock: &mut std::net::TcpStream, code: u16, ctype: &str, payload: &str) {
    let resp = format!(
        "HTTP/1.1 {code} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    let _ = sock.write_all(resp.as_bytes());
}

/// OpenAI-compatible mock: each request gets the next reply (SSE when asked).
fn mock_llm(replies: Vec<Value>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for reply in replies {
            let (mut sock, _) = listener.accept().unwrap();
            let Some(body) = read_body(&mut sock) else {
                continue;
            };
            let body: Value = serde_json::from_str(&body).unwrap();
            if body["stream"] == true {
                let mut delta = reply["choices"][0]["message"].clone();
                if let Some(calls) = delta.get_mut("tool_calls").and_then(|c| c.as_array_mut()) {
                    for (i, c) in calls.iter_mut().enumerate() {
                        c["index"] = json!(i);
                    }
                }
                let chunk = json!({"id": "c", "model": "m",
                    "choices": [{"delta": delta, "finish_reason": reply["choices"][0]["finish_reason"]}]});
                respond(
                    &mut sock,
                    200,
                    "text/event-stream",
                    &format!("data: {chunk}\n\ndata: [DONE]\n\n"),
                );
            } else {
                respond(&mut sock, 200, "application/json", &reply.to_string());
            }
        }
    });
    format!("http://127.0.0.1:{port}/v1")
}

/// What the mock judge does with one request.
enum Jev {
    /// Replay (or record) this fixture.
    Fixture(&'static str),
    /// Replay this fixture with its response edited.
    Mutated(&'static str, fn(&mut Value)),
    /// Answer with this HTTP status.
    Status(u16),
}

/// Mock of `POST {base}/systemone`. Paths under `tmp` become `<tmp>`.
fn mock_jev(replies: Vec<Jev>, tmp: &Path) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let tmp_paths: Vec<String> = [tmp.to_path_buf(), tmp.canonicalize().unwrap()]
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    std::thread::spawn(move || {
        for reply in replies {
            let (mut sock, _) = listener.accept().unwrap();
            let Some(mut body) = read_body(&mut sock) else {
                continue;
            };
            for p in &tmp_paths {
                body = body.replace(p.as_str(), "<tmp>");
            }
            let request: Value = serde_json::from_str(&body).unwrap();
            let (name, edit) = match reply {
                Jev::Status(code) => {
                    respond(&mut sock, code, "application/json", r#"{"error":"mock"}"#);
                    continue;
                }
                Jev::Fixture(n) => (n, None),
                Jev::Mutated(n, f) => (n, Some(f)),
            };
            match Recorded::from_env(fixture(name)).exchange(&request) {
                Ok(mut response) => {
                    if let Some(f) = edit {
                        f(&mut response);
                    }
                    respond(&mut sock, 200, "application/json", &response.to_string());
                }
                Err(e) => respond(&mut sock, 503, "text/plain", &e.to_string()),
            }
        }
    });
    format!("http://127.0.0.1:{port}/api/v1")
}

fn tempdir() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rung-turncheck-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn agent(tmp: &Path, llm: &str, jev: Option<&str>) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rung-agent"));
    c.current_dir(tmp)
        .env("HOME", tmp)
        .env("XDG_CONFIG_HOME", tmp)
        .env("RUNG_CONFIG", tmp.join("none.yaml"))
        .env("RUNG_HOME", tmp)
        .env("RUNG_BASE_URL", llm)
        .env("RUNG_MODEL", "m")
        .env("RUNG_API_KEY", "k")
        .env("RUNG_PROTOCOL", "openai")
        // The judge's key: a dummy, so a recording run's real key never
        // reaches the child or the mock.
        .env("OPENROUTER_API_KEY", "test-judge-key")
        .env_remove("RUNG_KEY_FILE")
        .env_remove("RUNG_SYSTEM_PROMPT_FILE")
        .env_remove("RUNG_REASONING")
        .env_remove("RUNG_TURN_CHECK")
        .env_remove("RUNG_TURN_CHECK_BASE_URL");
    if let Some(url) = jev {
        c.env("RUNG_TURN_CHECK", "jev")
            .env("RUNG_TURN_CHECK_BASE_URL", url);
    }
    c
}

struct Run {
    out: Value,
    stdout: String,
    stderr: String,
}

fn run(mut c: Command, tools: &str, prompt: &str) -> Run {
    let o = c
        .args(["--tools", tools, "--json", prompt])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(o.status.success(), "exit {:?}\n{stderr}", o.status);
    let out = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    Run {
        out,
        stdout,
        stderr,
    }
}

fn session_status(tmp: &Path, id: &str) -> String {
    let p = tmp.join(".rung/sessions").join(format!("{id}.json"));
    let s: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    s["status"].as_str().unwrap().to_string()
}

// ─── 1 · The #128 regression: narrate → nudge → act ─────────────────────────

/// The model only writes that it kept the note. The judge reads narration,
/// the loop is nudged once, the model calls `write_file`, and the turn
/// completes with the file on disk.
#[test]
fn a_narrated_turn_is_nudged_and_completes_when_it_acts() {
    let tmp = tempdir();
    let llm = mock_llm(vec![
        text_reply(NARRATION),
        write_note_reply(),
        text_reply(NARRATION),
    ]);
    let jev = mock_jev(
        vec![Jev::Fixture("narrated_note"), Jev::Fixture("act_note")],
        &tmp,
    );
    let r = run(agent(&tmp, &llm, Some(&jev)), "write", NARRATE_REQUEST);
    assert_eq!(r.out["status"], "completed", "{}\n{}", r.stdout, r.stderr);
    assert_eq!(r.out["turn_check"]["nudged"], true);
    assert_eq!(r.out["turn_check"]["outcome"], "done");
    assert_eq!(r.out["api_calls"], 3);
    assert!(
        tmp.join("notes.txt").exists(),
        "the nudge made the note real"
    );
    let id = r.out["task_id"].as_str().unwrap();
    assert_eq!(session_status(&tmp, id), "completed");
    let _ = std::fs::remove_dir_all(&tmp);
}

/// The same turn, but the model narrates again after its nudge: unverified,
/// the text kept, no second nudge.
#[test]
fn a_turn_still_narrating_after_its_nudge_is_unverified() {
    let tmp = tempdir();
    let llm = mock_llm(vec![text_reply(NARRATION), text_reply(NARRATION)]);
    let jev = mock_jev(
        vec![Jev::Fixture("narrated_note"), Jev::Fixture("narrated_note")],
        &tmp,
    );
    let r = run(agent(&tmp, &llm, Some(&jev)), "write", NARRATE_REQUEST);
    assert_eq!(r.out["status"], "unverified", "{}\n{}", r.stdout, r.stderr);
    assert_eq!(r.out["text"], NARRATION);
    assert_eq!(r.out["turn_check"]["nudged"], true);
    assert_eq!(r.out["turn_check"]["outcome"], "narrated");
    assert_eq!(r.out["api_calls"], 2, "one nudge, not two");
    assert!(!tmp.join("notes.txt").exists());
    let id = r.out["task_id"].as_str().unwrap();
    assert_eq!(session_status(&tmp, id), "unverified");
    let _ = std::fs::remove_dir_all(&tmp);
}

// ─── 6 · Mutation sibling of 1 ───────────────────────────────────────────────

/// The regression depends on the judge's answer. Flip the narration reading
/// (`claims_unperformed_action` 0.95 → 0.05, and the `outcome` Choice to
/// `done`) and the #128 symptom is back: "completed", no nudge, no file.
///
/// Flipping `claims_unperformed_action` alone is not enough, and a sibling
/// test pins that: the ask arm is an OR, and `outcome = narrated` with high
/// confidence still nudges.
#[test]
fn flipping_the_narration_reading_brings_the_bug_back() {
    let tmp = tempdir();
    let llm = mock_llm(vec![text_reply(NARRATION)]);
    let jev = mock_jev(vec![Jev::Mutated("narrated_note", flip_to_done)], &tmp);
    let r = run(agent(&tmp, &llm, Some(&jev)), "write", NARRATE_REQUEST);
    assert_eq!(r.out["status"], "completed", "{}", r.stderr);
    assert_eq!(r.out["turn_check"]["nudged"], false);
    assert_eq!(r.out["api_calls"], 1);
    assert!(
        !tmp.join("notes.txt").exists(),
        "completed with no file: #128"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

fn flip_to_done(r: &mut Value) {
    r["answers"]["claims_unperformed_action"]["noul"] = json!(0.05);
    let o = &mut r["answers"]["outcome"];
    o["choice"] = json!("done");
    o["confidence"] = json!(0.95);
    o["probabilities"] = json!({"done": 0.95, "answered": 0.02, "asked_user": 0.01, "blocked": 0.01, "narrated": 0.01});
}

#[test]
fn flipping_only_claims_unperformed_still_nudges() {
    let ask = narration_ask();
    let d = MutatedDecider {
        inner: Recorded::from_env(fixture("narrated_note")),
        edit: |r| r["answers"]["claims_unperformed_action"]["noul"] = json!(0.05),
    };
    let decided = d.decide(&ask).unwrap();
    assert_eq!(decided.noul("claims_unperformed_action"), Some(0.05));
    let reading = reading_of(&decided);
    assert_eq!(arm(&reading, Default::default()), turn_check::Arm::Ask);
}

// ─── 2 · The proven path ─────────────────────────────────────────────────────

#[test]
fn the_proven_path_completes_with_outcome_done() {
    let tmp = tempdir();
    let llm = mock_llm(vec![write_note_reply(), text_reply(NARRATION)]);
    let jev = mock_jev(vec![Jev::Fixture("act_note")], &tmp);
    let r = run(agent(&tmp, &llm, Some(&jev)), "write", NARRATE_REQUEST);
    assert_eq!(r.out["status"], "completed", "{}", r.stderr);
    assert_eq!(r.out["turn_check"]["outcome"], "done");
    assert_eq!(r.out["turn_check"]["nudged"], false);
    assert_eq!(r.out["turn_check"]["model"], "typesafe/jev-1.13-20260917");
    assert!(tmp.join("notes.txt").exists());
    let _ = std::fs::remove_dir_all(&tmp);
}

// ─── 3 · An information-only answer ──────────────────────────────────────────

#[test]
fn an_information_only_answer_completes_unflagged() {
    let tmp = tempdir();
    let llm = mock_llm(vec![text_reply(INFO_ANSWER)]);
    let jev = mock_jev(vec![Jev::Fixture("info_answer")], &tmp);
    let r = run(agent(&tmp, &llm, Some(&jev)), "none", INFO_REQUEST);
    assert_eq!(r.out["status"], "completed", "{}", r.stderr);
    assert_eq!(r.out["turn_check"]["outcome"], "answered");
    assert_eq!(r.out["turn_check"]["nudged"], false);
    assert!(r.out["turn_check"]["claims_unperformed"].as_f64().unwrap() <= 0.3);
    assert_eq!(r.out["api_calls"], 1);
    let _ = std::fs::remove_dir_all(&tmp);
}

// ─── 4 · No judge reachable ──────────────────────────────────────────────────

fn assert_unchecked(r: &Run, why: &str) {
    assert_eq!(r.out["status"], "unchecked", "{}\n{}", r.stdout, r.stderr);
    assert_eq!(r.out["text"], INFO_ANSWER, "the answer is delivered");
    let reason = r.out["turn_check"]["reason"].as_str().unwrap_or("");
    assert!(reason.contains(why), "reason `{reason}` lacks `{why}`");
    assert!(r.out["turn_check"].get("outcome").is_none());
}

#[test]
fn an_unreachable_judge_leaves_the_turn_unchecked() {
    let tmp = tempdir();
    // Bind and drop: the port refuses connections.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let llm = mock_llm(vec![text_reply(INFO_ANSWER)]);
    let url = format!("http://127.0.0.1:{port}/api/v1");
    let r = run(agent(&tmp, &llm, Some(&url)), "none", INFO_REQUEST);
    assert_unchecked(&r, "unreachable");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn a_401_or_402_leaves_the_turn_unchecked() {
    for (code, why) in [(401, "unauthorized"), (402, "no credit")] {
        let tmp = tempdir();
        let llm = mock_llm(vec![text_reply(INFO_ANSWER)]);
        let jev = mock_jev(vec![Jev::Status(code)], &tmp);
        let r = run(agent(&tmp, &llm, Some(&jev)), "none", INFO_REQUEST);
        assert_unchecked(&r, why);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

#[test]
fn no_key_leaves_the_turn_unchecked() {
    let tmp = tempdir();
    let llm = mock_llm(vec![text_reply(INFO_ANSWER)]);
    let jev = mock_jev(vec![Jev::Fixture("info_answer")], &tmp);
    let mut c = agent(&tmp, &llm, Some(&jev));
    c.env("OPENROUTER_API_KEY", "");
    let r = run(c, "none", INFO_REQUEST);
    assert_unchecked(&r, "no key");
    let _ = std::fs::remove_dir_all(&tmp);
}

// ─── 5 · A malformed answer ──────────────────────────────────────────────────

#[test]
fn a_malformed_answer_is_unchecked_never_completed() {
    type Edit = fn(&mut Value);
    let edits: [(Edit, &str); 3] = [
        (
            |r| {
                r["answers"].as_object_mut().unwrap().remove("outcome");
            },
            "not answered",
        ),
        (
            |r| r["answers"]["claims_unperformed_action"]["noul"] = json!(1.7),
            "outside [0,1]",
        ),
        (
            |r| r["answers"]["outcome"]["probabilities"]["answered"] = json!(0.3),
            "sum to",
        ),
    ];
    for (edit, why) in edits {
        let tmp = tempdir();
        let llm = mock_llm(vec![text_reply(INFO_ANSWER)]);
        let jev = mock_jev(vec![Jev::Mutated("info_answer", edit)], &tmp);
        let r = run(agent(&tmp, &llm, Some(&jev)), "none", INFO_REQUEST);
        assert_unchecked(&r, why);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

// ─── 8 · The state builder ───────────────────────────────────────────────────

fn tool_use(id: &str, name: &str, input: Value) -> ChatMessage {
    ChatMessage {
        role: "assistant".into(),
        content: MessageContent::Blocks(vec![MessageContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
            cache: None,
        }]),
    }
}

fn tool_result(id: &str, content: &str, is_error: bool) -> ChatMessage {
    ChatMessage {
        role: "user".into(),
        content: MessageContent::Blocks(vec![MessageContentBlock::ToolResult {
            tool_use_id: id.into(),
            content: content.into(),
            images: Vec::new(),
            is_error,
            cache: None,
        }]),
    }
}

/// 32 iterations of three large tool calls each, with a registered secret
/// in every kind of string: the state stays under the ceiling, elides the
/// middle, and never carries the secret.
#[test]
fn a_32_iteration_turn_stays_under_the_ceiling_and_is_redacted() {
    let secret = "sk-live-turncheck-7f3a9c";
    rung_agent::mcp::register_secret(secret);
    let big = format!("{secret} {}", "x".repeat(20_000));
    let mut turn = Vec::new();
    for i in 0..32 {
        for j in 0..3 {
            let id = format!("call_{i}_{j}");
            turn.push(tool_use(&id, "shell", json!({"command": big.clone()})));
            turn.push(tool_result(
                &id,
                &format!("{big}\n{secret}\n[exit: 0]"),
                false,
            ));
        }
    }
    let request = format!("{} {secret}", "please ".repeat(1000));
    let final_message = format!("done {secret} {}", "y".repeat(5000));
    let (ask, any_error) = turn_ask(&request, &turn, &[], &final_message);
    assert!(!any_error);
    assert!(
        ask.estimated_tokens() <= turn_check::STATE_TOKEN_LIMIT,
        "{} estimated tokens",
        ask.estimated_tokens()
    );
    assert_eq!(ask.state["actions"].as_array().unwrap().len(), 60);
    assert_eq!(ask.state["actions_elided"], 96 - 60);
    let text = ask.state.to_string();
    assert!(
        !text.contains(secret),
        "a registered secret reached the state"
    );
    assert!(text.contains("[REDACTED]"));
}

// ─── 9 · Off is what it was ──────────────────────────────────────────────────

/// With the check off, `--json` prints exactly what it printed before the
/// check existed, and nothing is asked of any judge.
#[test]
fn off_output_is_byte_identical_to_before() {
    for switch in [None, Some("off")] {
        let tmp = tempdir();
        let llm = mock_llm(vec![text_reply("pong")]);
        let mut c = agent(&tmp, &llm, None);
        if let Some(s) = switch {
            c.env("RUNG_TURN_CHECK", s);
        }
        c.args(["--task-id", "t-off"]);
        let r = run(c, "none", "say pong");
        assert_eq!(
            r.stdout,
            "{\"task_id\":\"t-off\",\"text\":\"pong\",\"status\":\"completed\",\"api_calls\":1,\"isolation_path\":null}\n"
        );
        assert_eq!(session_status(&tmp, "t-off"), "completed");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

/// `--stream` ends with the same status and reading as `--json`.
#[test]
fn the_stream_result_line_carries_the_reading() {
    let tmp = tempdir();
    let llm = mock_llm(vec![text_reply(INFO_ANSWER)]);
    let jev = mock_jev(vec![Jev::Fixture("info_answer")], &tmp);
    let o = agent(&tmp, &llm, Some(&jev))
        .args(["--tools", "none", "--stream", INFO_REQUEST])
        .output()
        .unwrap();
    assert!(o.status.success());
    let stdout = String::from_utf8_lossy(&o.stdout);
    let last: Value = serde_json::from_str(stdout.lines().last().unwrap()).unwrap();
    assert_eq!(last["type"], "result", "{stdout}");
    assert_eq!(last["response"]["status"], "completed");
    assert_eq!(last["response"]["api_calls"], 1);
    assert_eq!(last["response"]["turn_check"]["outcome"], "answered");
    let _ = std::fs::remove_dir_all(&tmp);
}

// ─── ACP: the reading rides `_meta.rung.turn_check` ─────────────────────────

fn acp_prompt(jev: Option<&str>, tmp: &Path, llm: &str) -> Value {
    let mut c = agent(tmp, llm, jev);
    let mut child = c
        .arg("--acp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut ask = |id: u32, method: &str, params: Value| -> Value {
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
        loop {
            let mut line = String::new();
            stdout.read_line(&mut line).unwrap();
            let v: Value = serde_json::from_str(line.trim()).unwrap();
            if v["id"] == id && v.get("method").is_none() {
                return v;
            }
        }
    };
    ask(1, "initialize", json!({"protocolVersion": 1}));
    let cwd = tmp.to_string_lossy().into_owned();
    let created = ask(2, "session/new", json!({"cwd": cwd, "mcpServers": []}));
    let sid = created["result"]["sessionId"].as_str().unwrap().to_string();
    let r = ask(
        3,
        "session/prompt",
        json!({"sessionId": sid, "prompt": [{"type": "text", "text": INFO_REQUEST}]}),
    );
    drop(stdin);
    let _ = child.wait();
    r
}

#[test]
fn acp_carries_the_reading_in_meta_and_ends_the_turn() {
    let tmp = tempdir();
    let llm = mock_llm(vec![text_reply(INFO_ANSWER)]);
    let jev = mock_jev(vec![Jev::Fixture("info_answer")], &tmp);
    let r = acp_prompt(Some(&jev), &tmp, &llm);
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    let meta = &r["result"]["_meta"]["rung"];
    assert_eq!(meta["status"], "completed", "{r}");
    assert_eq!(meta["turn_check"]["outcome"], "answered", "{r}");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn acp_with_the_check_off_has_no_meta() {
    let tmp = tempdir();
    let llm = mock_llm(vec![text_reply(INFO_ANSWER)]);
    let r = acp_prompt(None, &tmp, &llm);
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    assert!(r["result"].get("_meta").is_none(), "{r}");
    let _ = std::fs::remove_dir_all(&tmp);
}

// ─── Real transcripts, in process ────────────────────────────────────────────

/// Decider that edits the recorded response before reading it.
#[derive(Debug)]
struct MutatedDecider {
    inner: Recorded,
    edit: fn(&mut Value),
}

impl Decider for MutatedDecider {
    fn decide(&self, ask: &Ask) -> Result<Decided, Undecided> {
        let mut r = self.inner.exchange(&ask.body("typesafe/jev-1.13"))?;
        (self.edit)(&mut r);
        read_answers(ask, &r)
    }
}

fn reading_of(d: &Decided) -> TurnReading {
    let (outcome, confidence, _) = d.choice("outcome").unwrap();
    TurnReading {
        outcome: outcome.into(),
        confidence,
        claims_unperformed: d.noul("claims_unperformed_action").unwrap(),
        request_needs_action: d.noul("request_needs_action").unwrap(),
        hides_failure: d.noul("hides_failure").unwrap(),
        relies_on_prior_turn: d.noul("relies_on_prior_turn").unwrap(),
        model: d.model.clone(),
        cost_usd: d.usage.cost_usd,
    }
}

/// The state the binary builds for the narrated first turn.
fn narration_ask() -> Ask {
    let turn = vec![ChatMessage::assistant(NARRATION)];
    turn_ask(NARRATE_REQUEST, &turn, &[], NARRATION).0
}

/// A compact transcript fixture as the messages a turn adds.
fn turn_of(t: &Value) -> (String, Vec<ChatMessage>, String) {
    let mut turn = Vec::new();
    for (i, c) in t["calls"].as_array().unwrap().iter().enumerate() {
        let id = c["id"]
            .as_str()
            .map(str::to_string)
            .unwrap_or(format!("call_{i}"));
        turn.push(tool_use(
            &id,
            c["name"].as_str().unwrap(),
            c["input"].clone(),
        ));
        turn.push(tool_result(&id, c["result"].as_str().unwrap(), false));
    }
    let final_message = t["final_message"].as_str().unwrap().to_string();
    turn.push(ChatMessage::assistant(final_message.clone()));
    (
        t["request"].as_str().unwrap().to_string(),
        turn,
        final_message,
    )
}

/// Run the ladder on one turn with the named fixture as its judge.
fn check(name: &str, request: &str, turn: Vec<ChatMessage>, final_message: &str) -> Checked {
    let result = AgentResult {
        final_response: final_message.into(),
        transcript: turn,
        api_calls_made: 1,
        usage: Usage::default(),
        truncated: false,
        forced: false,
        elided: 0,
    };
    let carry = turncheck::Carry {
        decider: Arc::new(Recorded::from_env(fixture(name))),
        request: request.into(),
        prior_actions: Vec::new(),
    };
    match turncheck::step(turncheck::Ended::new(Turn::first(result, 0), carry)) {
        Ok(turncheck::StepOutcome::Completed(c)) => {
            Checked::Completed(c.into_payload().report().reading.clone().unwrap())
        }
        Ok(turncheck::StepOutcome::Nudge(n)) => Checked::Nudge(n.into_payload().reading().clone()),
        Ok(turncheck::StepOutcome::Unverified(f)) => {
            Checked::Unverified(f.into_payload().report().reading.clone().unwrap())
        }
        Ok(turncheck::StepOutcome::Unchecked(u)) => {
            Checked::Unchecked(u.into_payload().reason().unwrap_or("").to_string())
        }
        Err(f) => panic!("{}", f.error),
    }
}

#[derive(Debug)]
#[allow(dead_code)] // the readings are printed when a test fails
enum Checked {
    Completed(TurnReading),
    Nudge(TurnReading),
    Unverified(TurnReading),
    Unchecked(String),
}

/// A real worker turn (spire-venue agency probe A): a shell call that fails
/// (`No module named 'PIL'`), three Python calls that decode the PNG by hand,
/// and a shell call that writes and uploads the deliverable. The turn did
/// what it says. The upload token in the request is registered as a secret
/// and never reaches Jev.
///
/// Jev is not sure of it: `outcome` splits between done (0.39) and narrated
/// (0.40) at confidence 0.25, and `hides_failure` reads the worked-around
/// PIL error as hidden (0.67). The gate escalates. That is a false flag, and
/// it is the failure the gate is built to have: the turn is `unverified`,
/// never nudged on a guess and never called completed on a guess.
#[test]
fn a_real_messy_done_turn_is_escalated_not_nudged() {
    rung_agent::mcp::register_secret("local-test");
    let (request, turn, final_message) = turn_of(&transcript("probe_a_decode"));
    let (ask, any_error) = turn_ask(&request, &turn, &[], &final_message);
    assert!(any_error, "the PIL failure is an errored action");
    assert!(!ask.state.to_string().contains("token=local-test"));
    match check("probe_a_decode", &request, turn, &final_message) {
        Checked::Unverified(r) => {
            assert!(r.confidence < 0.7, "{r:?}");
            assert!(r.claims_unperformed < 0.8, "{r:?}");
        }
        other => panic!("{other:?}"),
    }
}

/// The blocked-with-error case, Phase 0's closest miss (0.72 there). The
/// final message honestly reports the failed push, and Jev reads `outcome`
/// as blocked at 0.99, but `claims_unperformed_action` is 0.74: above the
/// act ceiling (0.3), below the ask floor (0.8). The gate escalates rather
/// than nudging an honest report.
#[test]
fn the_blocked_with_error_case_is_escalated_not_nudged() {
    let (request, turn, final_message) = turn_of(&transcript("blocked_push_error"));
    let (_, any_error) = turn_ask(&request, &turn, &[], &final_message);
    assert!(any_error, "exit 128 is an error");
    match check("blocked_push_error", &request, turn, &final_message) {
        Checked::Unverified(r) => {
            assert_eq!(r.outcome, "blocked");
            assert!(
                r.claims_unperformed > 0.3 && r.claims_unperformed < 0.8,
                "{r:?}"
            );
        }
        other => panic!("{other:?}"),
    }
}

/// A long turn: the real probe-A actions cycled to 70 entries, so the state
/// elides the middle ten and is about 15K tokens. Jev answers it (no
/// refusal, no truncation); the reading is as unsure as the short one's, and
/// the gate escalates.
#[test]
fn a_long_state_is_judged_and_escalated() {
    let t = transcript("probe_a_decode");
    let (request, base, final_message) = turn_of(&t);
    let calls: Vec<ChatMessage> = base[..base.len() - 1].to_vec();
    let mut turn = Vec::new();
    let mut q: VecDeque<ChatMessage> = VecDeque::new();
    for i in 0..70 {
        for m in calls.iter().skip((i % 5) * 2).take(2) {
            let mut m = m.clone();
            if let MessageContent::Blocks(blocks) = &mut m.content {
                for b in blocks {
                    match b {
                        MessageContentBlock::ToolUse { id, .. } => *id = format!("long_{i}"),
                        MessageContentBlock::ToolResult { tool_use_id, .. } => {
                            *tool_use_id = format!("long_{i}")
                        }
                        _ => {}
                    }
                }
            }
            q.push_back(m);
        }
    }
    turn.extend(q);
    turn.push(ChatMessage::assistant(final_message.clone()));
    rung_agent::mcp::register_secret("local-test");
    let (ask, _) = turn_ask(&request, &turn, &[], &final_message);
    assert_eq!(ask.state["actions_elided"], 10);
    assert!(ask.estimated_tokens() > 8_000, "{}", ask.estimated_tokens());
    match check("long_state", &request, turn, &final_message) {
        Checked::Unverified(r) => assert!(r.confidence < 0.7, "{r:?}"),
        other => panic!("{other:?}"),
    }
}
