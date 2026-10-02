//! ACP v1 against `rung-agent --acp` (no LLM, or a local mock).

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::json;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rung-agent"))
}

fn read_json(reader: &mut BufReader<impl std::io::Read>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("stdout line");
    serde_json::from_str(line.trim()).unwrap_or_else(|e| panic!("json {e}: {line}"))
}

#[test]
fn initialize_new_list_set_mode_close() {
    let tmp = tempfile();
    let cwd = tmp.to_string_lossy().into_owned();
    let mut child = bin()
        .arg("--acp")
        .current_dir(&tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":1}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let init = read_json(&mut stdout);
    assert_eq!(init["result"]["protocolVersion"], 1);
    assert_eq!(init["result"]["agentCapabilities"]["loadSession"], true);
    let agent_caps = &init["result"]["agentCapabilities"];
    assert_eq!(agent_caps["promptCapabilities"]["image"], true);
    assert_eq!(agent_caps["promptCapabilities"]["audio"], true);
    assert_eq!(agent_caps["mcpCapabilities"]["http"], true);
    let caps = &agent_caps["sessionCapabilities"];
    assert!(caps["list"].is_object());
    assert!(caps["resume"].is_object());
    assert!(caps["fork"].is_object());

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":2,"method":"session/new","params":{{"cwd":"{cwd}","mcpServers":[]}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let created = read_json(&mut stdout);
    let sid = created["result"]["sessionId"].as_str().expect("sessionId");
    assert!(!sid.is_empty());
    let modes = &created["result"]["modes"]["availableModes"];
    assert_eq!(modes.as_array().map(|a| a.len()), Some(3));

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":3,"method":"session/list","params":{{"cwd":"{cwd}"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let listed = read_json(&mut stdout);
    assert_eq!(listed["result"]["sessions"].as_array().unwrap().len(), 1);

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":4,"method":"session/set_mode","params":{{"sessionId":"{sid}","modeId":"explore"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let mode = read_json(&mut stdout);
    assert!(mode.get("result").is_some(), "{mode}");

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":5,"method":"session/prompt","params":{{"sessionId":"{sid}","prompt":[]}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let prompt = read_json(&mut stdout);
    assert_eq!(prompt["result"]["stopReason"], "end_turn");

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"session/cancel","params":{{"sessionId":"{sid}"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":6,"method":"session/close","params":{{"sessionId":"{sid}"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let closed = read_json(&mut stdout);
    assert!(closed.get("result").is_some(), "{closed}");

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success(), "{status:?}");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn fork_resume_does_not_replay_load_does() {
    let tmp = tempfile();
    let cwd = tmp.to_string_lossy().into_owned();
    let mut child = bin()
        .arg("--acp")
        .current_dir(&tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":1}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let _ = read_json(&mut stdout);

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":2,"method":"session/new","params":{{"cwd":"{cwd}","mcpServers":[]}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let created = read_json(&mut stdout);
    let sid = created["result"]["sessionId"].as_str().expect("sessionId");

    let path = tmp
        .join(".rung")
        .join("sessions")
        .join(format!("{sid}.json"));
    let mut sess: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    sess["lines"] = json!([
        {"role": "user", "text": "hi"},
        {"role": "assistant", "text": "secret-replay"}
    ]);
    std::fs::write(&path, serde_json::to_string_pretty(&sess).unwrap()).unwrap();

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":3,"method":"session/fork","params":{{"sessionId":"{sid}","cwd":"{cwd}","mcpServers":[]}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let forked = read_json(&mut stdout);
    let fid = forked["result"]["sessionId"].as_str().expect("fork id");
    assert_ne!(fid, sid);
    assert_eq!(forked["result"]["modes"]["currentModeId"], "implement");

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":4,"method":"session/list","params":{{"cwd":"{cwd}"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let listed = read_json(&mut stdout);
    assert_eq!(listed["result"]["sessions"].as_array().unwrap().len(), 2);

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":5,"method":"session/resume","params":{{"sessionId":"{sid}","cwd":"{cwd}","mcpServers":[]}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let resume = read_json(&mut stdout);
    assert!(resume.get("result").is_some(), "{resume}");
    assert!(
        resume.get("method").is_none(),
        "resume must not replay: {resume}"
    );
    assert_eq!(resume["result"]["modes"]["currentModeId"], "implement");

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":6,"method":"session/load","params":{{"sessionId":"{sid}","cwd":"{cwd}","mcpServers":[]}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let load_note = read_json(&mut stdout);
    assert_eq!(load_note["method"], "session/update", "{load_note}");
    let replay = load_note["params"]["update"]["content"]["text"]
        .as_str()
        .unwrap_or_default();
    assert_eq!(replay, "secret-replay");
    let load = read_json(&mut stdout);
    assert!(load.get("result").is_some(), "{load}");

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success(), "{status:?}");
    let _ = std::fs::remove_dir_all(&tmp);
}

/// One-shot OpenAI-compatible server: answers each request with the next
/// canned reply (SSE when the body asks to stream) and records the bodies.
fn mock_llm(
    replies: Vec<serde_json::Value>,
) -> (String, std::sync::mpsc::Receiver<serde_json::Value>) {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for reply in replies {
            let (mut sock, _) = listener.accept().unwrap();
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
                            let l = l.to_ascii_lowercase();
                            l.strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if buf.len() >= at + 4 + len {
                        break String::from_utf8_lossy(&buf[at + 4..at + 4 + len]).to_string();
                    }
                }
                if n == 0 {
                    panic!("short request");
                }
            };
            let body: serde_json::Value = serde_json::from_str(&body).unwrap();
            let stream = body["stream"] == true;
            tx.send(body).unwrap();
            // `__delay_ms` holds the reply back; `__status` answers with that
            // HTTP status and `__body` instead of a completion.
            if let Some(ms) = reply["__delay_ms"].as_u64() {
                std::thread::sleep(std::time::Duration::from_millis(ms));
            }
            let status = reply["__status"].as_u64().unwrap_or(200);
            let (ctype, payload) = if status != 200 {
                ("application/json", reply["__body"].to_string())
            } else if stream {
                let mut delta = reply["choices"][0]["message"].clone();
                if let Some(calls) = delta.get_mut("tool_calls").and_then(|c| c.as_array_mut()) {
                    for (i, c) in calls.iter_mut().enumerate() {
                        c["index"] = json!(i);
                    }
                }
                let chunk = json!({
                    "id": reply["id"], "model": reply["model"],
                    "choices": [{"delta": delta, "finish_reason": reply["choices"][0]["finish_reason"]}]
                });
                (
                    "text/event-stream",
                    format!("data: {chunk}\n\ndata: [DONE]\n\n"),
                )
            } else {
                ("application/json", reply.to_string())
            };
            let resp = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}"), rx)
}

fn text_reply(text: &str) -> serde_json::Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": text}, "finish_reason": "stop"}]})
}

// ─── Terminal states on the wire ─────────────────────────────────────────────
//
// One turn per test, launched the way the Spire CONTROL host does
// (`--acp --tools none`, env isolated). Each asserts the exact
// `session/prompt` response so a wire change shows here.

fn shell_call() -> serde_json::Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": "shell", "arguments": "{\"command\":\"ls\"}"}}
    ]}, "finish_reason": "tool_calls"}]})
}

struct Turn {
    /// The `session/prompt` response line.
    response: serde_json::Value,
    /// `agent_message_chunk` texts sent during the turn.
    said: Vec<String>,
    /// Request bodies the model saw.
    bodies: Vec<serde_json::Value>,
}

/// Run one ACP prompt against `replies`. With `cancel_after`, send
/// `session/cancel` that long after the prompt.
fn one_turn(
    replies: Vec<serde_json::Value>,
    args: &[&str],
    cancel_after: Option<std::time::Duration>,
) -> Turn {
    let tmp = tempfile();
    let cwd = tmp.to_string_lossy().into_owned();
    let (url, bodies) = mock_llm(replies);
    let mut child = bin()
        .arg("--acp")
        .args(args)
        .current_dir(&tmp)
        .env("HOME", &tmp)
        .env("XDG_CONFIG_HOME", &tmp)
        .env("RUNG_CONFIG", tmp.join("none.yaml"))
        .env("RUNG_HOME", &tmp)
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
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut said = Vec::new();
    let mut send = |msg: serde_json::Value| {
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
    };
    let mut wait = |id: u32, said: &mut Vec<String>| loop {
        let v = read_json(&mut stdout);
        if v["id"] == id && v.get("method").is_none() {
            return v;
        }
        let update = &v["params"]["update"];
        if update["sessionUpdate"] == "agent_message_chunk" {
            said.push(
                update["content"]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
            );
        }
    };
    let rpc = |id: u32, method: &str, params: serde_json::Value| json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    send(rpc(1, "initialize", json!({"protocolVersion": 1})));
    wait(1, &mut said);
    send(rpc(2, "session/new", json!({"cwd": cwd, "mcpServers": []})));
    let created = wait(2, &mut said);
    let sid = created["result"]["sessionId"].as_str().unwrap().to_string();
    let prompt = json!({"sessionId": sid, "prompt": [{"type": "text", "text": "do it"}]});
    send(rpc(3, "session/prompt", prompt));
    if let Some(after) = cancel_after {
        std::thread::sleep(after);
        send(json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sid}}));
    }
    let response = wait(3, &mut said);
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&tmp);
    Turn {
        response,
        said,
        bodies: bodies.try_iter().collect(),
    }
}

/// A model that finishes on its own: `end_turn`, nothing else (unchanged).
#[test]
fn terminal_end_turn_is_unchanged() {
    let t = one_turn(vec![text_reply("done")], &["--tools", "none"], None);
    assert_eq!(t.response["result"], json!({"stopReason": "end_turn"}));
    assert_eq!(t.said, ["done"]);
}

/// The cap withdrew the tools on the last call and the model answered: the
/// answer arrives, but the turn is `max_turn_requests`, not `end_turn`.
#[test]
fn terminal_cap_forced_answer_is_max_turn_requests() {
    let t = one_turn(
        vec![shell_call(), text_reply("forced summary")],
        &["--tools", "none", "--max-iterations", "2"],
        None,
    );
    assert!(t.bodies[1].get("tools").is_none(), "{}", t.bodies[1]);
    assert_eq!(
        t.response["result"],
        json!({"stopReason": "max_turn_requests", "_meta": {"rung": {"terminal": {
            "state": "cap_forced",
            "reason": "iteration cap (2) reached; the last call had no tools",
        }}}}),
        "{}",
        t.response
    );
    assert_eq!(t.said, ["forced summary"]);
}

/// The cap ran out with no answer at all.
#[test]
fn terminal_cap_exhausted_is_max_turn_requests() {
    let t = one_turn(
        vec![shell_call()],
        &["--tools", "none", "--max-iterations", "1"],
        None,
    );
    assert_eq!(
        t.response["result"],
        json!({"stopReason": "max_turn_requests", "_meta": {"rung": {"terminal": {
            "state": "cap_exhausted",
            "reason": "max iterations (1)",
        }}}}),
        "{}",
        t.response
    );
    assert!(t.said.is_empty(), "{:?}", t.said);
}

/// `session/cancel` mid-turn: `cancelled`, nothing else (unchanged).
#[test]
fn terminal_cancelled_is_unchanged() {
    let mut slow = text_reply("late");
    slow["__delay_ms"] = json!(1500);
    let t = one_turn(
        vec![slow],
        &["--tools", "none"],
        Some(std::time::Duration::from_millis(300)),
    );
    assert_eq!(t.response["result"], json!({"stopReason": "cancelled"}));
}

/// A model refusal is ACP `refusal` with the model's reason, not -32603.
#[test]
fn terminal_refusal_is_refusal() {
    let refusal = json!({"id": "c", "model": "m", "choices": [{"message": {
        "content": null, "refusal": "I can't help with that."
    }, "finish_reason": "stop"}]});
    let t = one_turn(vec![refusal], &["--tools", "none"], None);
    assert_eq!(
        t.response["result"],
        json!({"stopReason": "refusal", "_meta": {"rung": {"terminal": {
            "state": "refused",
            "reason": "model refused the request: I can't help with that.",
        }}}}),
        "{}",
        t.response
    );
}

/// Cut off by the token limit: `max_tokens`, nothing else (unchanged).
#[test]
fn terminal_truncated_is_unchanged() {
    let cut = json!({"id": "c", "model": "m", "choices": [{"message": {"content": "half"}, "finish_reason": "length"}]});
    let t = one_turn(vec![cut], &["--tools", "none"], None);
    assert_eq!(t.response["result"], json!({"stopReason": "max_tokens"}));
    assert_eq!(t.said, ["half"]);
}

/// The provider's context window is exceeded: an error whose data names
/// the state, not prose alone.
#[test]
fn terminal_overflow_is_typed_error() {
    let overflow = json!({"__status": 400, "__body": {"error": {
        "message": "This model's maximum context length is 8192 tokens",
        "code": "context_length_exceeded",
    }}});
    let t = one_turn(vec![overflow], &["--tools", "none"], None);
    let error = &t.response["error"];
    assert_eq!(error["code"], -32603, "{}", t.response);
    assert_eq!(error["message"], "Internal error");
    let terminal = &error["data"]["rung"]["terminal"];
    assert_eq!(terminal["state"], "overflow", "{}", t.response);
    assert!(
        terminal["reason"]
            .as_str()
            .unwrap()
            .starts_with("invalid-request (context-overflow): "),
        "{}",
        t.response
    );
    assert_eq!(terminal.as_object().unwrap().len(), 2, "{terminal}");
}

/// The same call with the same fast result, again and again: the doom
/// guard stops the turn and the error says so in `state`.
#[test]
fn terminal_doom_loop_is_typed_error() {
    let t = one_turn(vec![shell_call(); 4], &["--tools", "none"], None);
    assert_eq!(
        t.response["error"],
        json!({"code": -32603, "message": "Internal error", "data": {"rung": {"terminal": {
            "state": "doom_loop",
            "reason": "repeated shell with the same input and no progress",
        }}}}),
        "{}",
        t.response
    );
}

/// Any other unrecoverable failure is `failed`, with its kind.
#[test]
fn terminal_auth_failure_is_typed_error() {
    let denied = json!({"__status": 401, "__body": {"error": {"message": "bad key"}}});
    let t = one_turn(vec![denied], &["--tools", "none"], None);
    let error = &t.response["error"];
    assert_eq!(error["code"], -32603, "{}", t.response);
    let terminal = &error["data"]["rung"]["terminal"];
    assert_eq!(terminal["state"], "failed", "{}", t.response);
    assert_eq!(terminal["kind"], "auth", "{}", t.response);
    assert!(
        terminal["reason"].as_str().unwrap().starts_with("auth: "),
        "{}",
        t.response
    );
}

/// Turn 2's request carries turn 1's tool-use and tool-result, not only its
/// final text (#128).
#[test]
fn second_turn_replays_first_turn_tool_calls() {
    let tmp = tempfile();
    let cwd = tmp.to_string_lossy().into_owned();
    std::fs::write(tmp.join("marker.txt"), "x").unwrap();
    let tool_call = json!({"id": "c", "model": "m", "choices": [{"message": {"tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": "list_files", "arguments": "{\"path\":\".\"}"}}
    ]}, "finish_reason": "tool_calls"}]});
    let (url, bodies) = mock_llm(vec![tool_call, text_reply("listed"), text_reply("again")]);
    let mut child = bin()
        .arg("--acp")
        .current_dir(&tmp)
        .env("HOME", &tmp)
        .env("XDG_CONFIG_HOME", &tmp)
        .env("RUNG_CONFIG", tmp.join("none.yaml"))
        .env("RUNG_HOME", &tmp)
        .env("RUNG_BASE_URL", &url)
        .env("RUNG_MODEL", "m")
        .env("RUNG_API_KEY", "k")
        .env("RUNG_PROTOCOL", "openai")
        .env_remove("RUNG_KEY_FILE")
        .env_remove("RUNG_SYSTEM_PROMPT_FILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut ask = |id: u32, method: &str, params: serde_json::Value| -> serde_json::Value {
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
        loop {
            let v = read_json(&mut stdout);
            if v["id"] == id && v.get("method").is_none() {
                return v;
            }
        }
    };
    ask(1, "initialize", json!({"protocolVersion": 1}));
    let created = ask(2, "session/new", json!({"cwd": cwd, "mcpServers": []}));
    let sid = created["result"]["sessionId"].as_str().unwrap().to_string();
    let prompt = |t: &str| json!({"sessionId": sid, "prompt": [{"type": "text", "text": t}]});
    let r1 = ask(3, "session/prompt", prompt("list the files"));
    assert_eq!(r1["result"]["stopReason"], "end_turn", "{r1}");
    let r2 = ask(4, "session/prompt", prompt("and again"));
    assert_eq!(r2["result"]["stopReason"], "end_turn", "{r2}");

    let _ = bodies.recv().unwrap();
    let _ = bodies.recv().unwrap();
    let turn2 = bodies.recv().unwrap();
    let msgs = turn2["messages"].as_array().unwrap();
    let called = msgs.iter().any(|m| {
        m["role"] == "assistant" && m["tool_calls"][0]["function"]["name"] == "list_files"
    });
    let result = msgs.iter().any(|m| {
        m["role"] == "tool"
            && m["tool_call_id"] == "call_1"
            && m["content"].to_string().contains("marker.txt")
    });
    assert!(called, "turn 2 lost turn 1's tool call: {turn2}");
    assert!(result, "turn 2 lost turn 1's tool result: {turn2}");
    let last_assistant = msgs
        .iter()
        .rev()
        .find(|m| m["role"] == "assistant")
        .unwrap();
    assert_eq!(last_assistant["content"], "listed");

    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Two ACP prompts on one session under `--tools none` where turn 1 ends in
/// an error. Returns turn 1's JSON-RPC response, turn 2's request body and
/// the session file after turn 2.
fn failed_then_asked_again(
    turn1: Vec<serde_json::Value>,
) -> (serde_json::Value, serde_json::Value, serde_json::Value) {
    let tmp = tempfile();
    let cwd = tmp.to_string_lossy().into_owned();
    let calls = turn1.len();
    let mut replies = turn1;
    replies.push(text_reply("second answer"));
    let (url, bodies) = mock_llm(replies);
    let mut child = bin()
        .args(["--acp", "--tools", "none"])
        .current_dir(&tmp)
        .env("HOME", &tmp)
        .env("XDG_CONFIG_HOME", &tmp)
        .env("RUNG_CONFIG", tmp.join("none.yaml"))
        .env("RUNG_HOME", &tmp)
        .env("RUNG_BASE_URL", &url)
        .env("RUNG_MODEL", "m")
        .env("RUNG_API_KEY", "k")
        .env("RUNG_PROTOCOL", "openai")
        .env_remove("RUNG_KEY_FILE")
        .env_remove("RUNG_SYSTEM_PROMPT_FILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut ask = |id: u32, method: &str, params: serde_json::Value| -> serde_json::Value {
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
        loop {
            let v = read_json(&mut stdout);
            if v["id"] == id && v.get("method").is_none() {
                return v;
            }
        }
    };
    ask(1, "initialize", json!({"protocolVersion": 1}));
    let created = ask(2, "session/new", json!({"cwd": cwd, "mcpServers": []}));
    let sid = created["result"]["sessionId"].as_str().unwrap().to_string();
    let prompt = |t: &str| json!({"sessionId": sid, "prompt": [{"type": "text", "text": t}]});
    let r1 = ask(3, "session/prompt", prompt("do it"));
    let r2 = ask(4, "session/prompt", prompt("again"));
    assert_eq!(r2["result"]["stopReason"], "end_turn", "{r2}");
    for _ in 0..calls {
        let _ = bodies.recv().unwrap();
    }
    let turn2 = bodies.recv().unwrap();
    drop(stdin);
    let _ = child.wait();
    let file = tmp.join(".rung/sessions").join(format!("{sid}.json"));
    let session = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
    (r1, turn2, session)
}

/// The assistant messages a request body replays, as text.
fn assistant_texts(body: &serde_json::Value) -> Vec<String> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "assistant")
        .filter_map(|m| m["content"].as_str().map(str::to_string))
        .collect()
}

/// A turn stopped by the doom guard keeps the tool calls it made, and its
/// failure is not replayed as something the assistant said (E2).
#[test]
fn doom_stopped_turn_keeps_its_calls_and_is_not_replayed_as_speech() {
    let shell = |n: u32| {
        json!({"id": "c", "model": "m", "choices": [{"message": {"tool_calls": [
            {"id": format!("call_{n}"), "type": "function", "function": {"name": "shell", "arguments": "{\"command\":\"ls\"}"}}
        ]}, "finish_reason": "tool_calls"}]})
    };
    let (r1, turn2, session) = failed_then_asked_again((1..=4).map(shell).collect());
    let why = "repeated shell with the same input and no progress";
    assert_eq!(r1["error"]["data"], why, "{r1}");

    assert!(
        !assistant_texts(&turn2).iter().any(|t| t.contains(why)),
        "turn 2 replays the failure as assistant speech: {turn2}"
    );
    let msgs = turn2["messages"].as_array().unwrap();
    for n in 1..=3 {
        let id = format!("call_{n}");
        assert!(
            msgs.iter()
                .any(|m| m["role"] == "assistant" && m["tool_calls"][0]["id"] == id),
            "turn 2 lost turn 1's {id}: {turn2}"
        );
        assert!(
            msgs.iter()
                .any(|m| m["role"] == "tool" && m["tool_call_id"] == id),
            "turn 2 lost turn 1's result for {id}: {turn2}"
        );
    }
    // The stopped call never ran, so it is not in history.
    assert!(!turn2.to_string().contains("call_4"), "{turn2}");

    let lines = session["lines"].as_array().unwrap();
    let failed = &lines[1];
    assert_eq!(failed["role"], "assistant", "{session}");
    assert_eq!(failed["failure"], why, "{session}");
    assert_eq!(failed["text"], "", "{session}");
}

/// A refusal is recorded beside the turn, not replayed as the assistant's
/// own words (E3).
#[test]
fn refused_turn_is_not_replayed_as_speech() {
    let refusal = json!({"id": "c", "model": "m", "choices": [{"message": {
        "content": null, "refusal": "I can't help with that."
    }, "finish_reason": "stop"}]});
    let (r1, turn2, session) = failed_then_asked_again(vec![refusal]);
    let why = "model refused the request: I can't help with that.";
    assert_eq!(r1["error"]["data"], why, "{r1}");

    assert!(
        !turn2.to_string().contains("refused"),
        "turn 2 replays the refusal: {turn2}"
    );
    let roles: Vec<_> = turn2["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["role"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(roles, ["user", "user"], "{turn2}");
    assert_eq!(session["lines"][1]["failure"], why, "{session}");
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

/// Two ACP turns where turn 1 reads `frame.png`. Returns the recorded
/// request bodies: turn 1's tool round, then turn 2.
fn read_frame_turns(images: Option<&str>) -> (serde_json::Value, serde_json::Value) {
    let tmp = tempfile();
    let cwd = tmp.to_string_lossy().into_owned();
    std::fs::write(tmp.join("frame.png"), PNG).unwrap();
    let tool_call = json!({"id": "c", "model": "m", "choices": [{"message": {"tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"frame.png\"}"}}
    ]}, "finish_reason": "tool_calls"}]});
    let (url, bodies) = mock_llm(vec![tool_call, text_reply("seen"), text_reply("again")]);
    let mut cmd = bin();
    cmd.arg("--acp")
        .current_dir(&tmp)
        .env("HOME", &tmp)
        .env("XDG_CONFIG_HOME", &tmp)
        .env("RUNG_CONFIG", tmp.join("none.yaml"))
        .env("RUNG_HOME", &tmp)
        .env("RUNG_BASE_URL", &url)
        .env("RUNG_MODEL", "m")
        .env("RUNG_API_KEY", "k")
        .env("RUNG_PROTOCOL", "openai")
        .env_remove("RUNG_IMAGES")
        .env_remove("RUNG_KEY_FILE")
        .env_remove("RUNG_SYSTEM_PROMPT_FILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(v) = images {
        cmd.env("RUNG_IMAGES", v);
    }
    let mut child = cmd.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut ask = |id: u32, method: &str, params: serde_json::Value| -> serde_json::Value {
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
        loop {
            let v = read_json(&mut stdout);
            if v["id"] == id && v.get("method").is_none() {
                return v;
            }
        }
    };
    ask(1, "initialize", json!({"protocolVersion": 1}));
    let created = ask(2, "session/new", json!({"cwd": cwd, "mcpServers": []}));
    let sid = created["result"]["sessionId"].as_str().unwrap().to_string();
    let prompt = |t: &str| json!({"sessionId": sid, "prompt": [{"type": "text", "text": t}]});
    let r1 = ask(3, "session/prompt", prompt("look at frame.png"));
    assert_eq!(r1["result"]["stopReason"], "end_turn", "{r1}");
    let r2 = ask(4, "session/prompt", prompt("and again"));
    assert_eq!(r2["result"]["stopReason"], "end_turn", "{r2}");
    let _ = bodies.recv().unwrap();
    let tool_round = bodies.recv().unwrap();
    let turn2 = bodies.recv().unwrap();
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&tmp);
    (tool_round, turn2)
}

fn image_urls(body: &serde_json::Value) -> Vec<String> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["content"].as_array())
        .flatten()
        .filter(|p| p["type"] == "image_url")
        .map(|p| p["image_url"]["url"].as_str().unwrap().to_string())
        .collect()
}

/// With images on, the image `read_file` returned is in the next request
/// as an image part right after the tool message, and is not replayed
/// from session history on the next turn.
#[test]
fn tool_result_image_is_sent_to_a_model_that_takes_images() {
    let (tool_round, turn2) = read_frame_turns(Some("on"));
    let data = rung_std::llm::ImageSource::from_bytes(PNG).unwrap().data;
    let msgs = tool_round["messages"].as_array().unwrap();
    let at = msgs
        .iter()
        .position(|m| m["role"] == "tool" && m["tool_call_id"] == "call_1")
        .expect("tool message");
    assert!(
        msgs[at]["content"]
            .as_str()
            .unwrap()
            .contains("(image/png, 16 bytes)"),
        "{tool_round}"
    );
    assert_eq!(msgs[at + 1]["role"], "user", "{tool_round}");
    assert_eq!(
        image_urls(&tool_round),
        vec![format!("data:image/png;base64,{data}")]
    );

    assert!(image_urls(&turn2).is_empty(), "{turn2}");
    assert!(
        turn2.to_string().contains("not kept in session history"),
        "{turn2}"
    );
    assert!(!turn2.to_string().contains(&data), "{turn2}");
}

/// Images are off by default: a text-only model gets a note, not the image.
#[test]
fn tool_result_image_is_a_note_for_a_text_only_model() {
    let (tool_round, _) = read_frame_turns(None);
    let data = rung_std::llm::ImageSource::from_bytes(PNG).unwrap().data;
    assert!(image_urls(&tool_round).is_empty(), "{tool_round}");
    let tool = tool_round["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "tool")
        .unwrap();
    let content = tool["content"].as_str().unwrap();
    assert!(
        content
            .contains("[image omitted: image/png, 16 bytes; this model is set to take text only"),
        "{content}"
    );
    assert!(!tool_round.to_string().contains(&data), "{tool_round}");
}

fn tempfile() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rung-agent-acp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// The session's cwd, not the launch cwd, owns its store: prompt, set_mode,
/// close, delete all resolve through the cwd given at new/load/resume/fork
/// even when the process runs elsewhere (E7).
#[test]
fn session_ops_follow_session_cwd_not_process_cwd() {
    let tmp = tempfile();
    let launch = tmp.join("launch");
    let work = tmp.join("work");
    std::fs::create_dir_all(&launch).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    let (url, _bodies) = mock_llm(vec![
        text_reply("one"),
        text_reply("two"),
        text_reply("three"),
    ]);
    let spawn = || {
        bin()
            .arg("--acp")
            .current_dir(&launch)
            .env("HOME", &tmp)
            .env("XDG_CONFIG_HOME", &tmp)
            .env("RUNG_CONFIG", tmp.join("none.yaml"))
            .env("RUNG_HOME", &tmp)
            .env("RUNG_BASE_URL", &url)
            .env("RUNG_MODEL", "m")
            .env("RUNG_API_KEY", "k")
            .env("RUNG_PROTOCOL", "openai")
            .env_remove("RUNG_KEY_FILE")
            .env_remove("RUNG_SYSTEM_PROMPT_FILE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    };
    let wd = work.to_string_lossy().into_owned();
    let sess_file = |id: &str| work.join(".rung/sessions").join(format!("{id}.json"));
    let read = |id: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(sess_file(id)).unwrap()).unwrap()
    };

    // Process 1: new in `work`, two prompts, set_mode, close.
    let mut child = spawn();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut n = 0u32;
    let mut ask = |method: &str, params: serde_json::Value| -> serde_json::Value {
        n += 1;
        let id = n;
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
        loop {
            let v = read_json(&mut stdout);
            if v["id"] == id && v.get("method").is_none() {
                return v;
            }
        }
    };
    ask("initialize", json!({"protocolVersion": 1}));
    let created = ask("session/new", json!({"cwd": wd, "mcpServers": []}));
    let sid = created["result"]["sessionId"].as_str().unwrap().to_string();
    let prompt = |t: &str| json!({"sessionId": sid, "prompt": [{"type": "text", "text": t}]});
    ask("session/prompt", prompt("first"));
    ask("session/prompt", prompt("second"));
    let after = read(&sid);
    assert_eq!(after["lines"].as_array().unwrap().len(), 4, "{after}");
    assert!(
        !launch.join(".rung/sessions").exists(),
        "session forked into the launch cwd"
    );
    ask(
        "session/set_mode",
        json!({"sessionId": sid, "modeId": "review"}),
    );
    assert_eq!(read(&sid)["kind"], "review");
    ask("session/close", json!({"sessionId": sid}));
    assert_eq!(read(&sid)["status"], "closed");
    drop(stdin);
    let _ = child.wait();

    // Process 2: resume in `work` (fresh process, launch cwd elsewhere), prompt.
    let mut child = spawn();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut n = 0u32;
    let mut ask = |method: &str, params: serde_json::Value| -> serde_json::Value {
        n += 1;
        let id = n;
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
        loop {
            let v = read_json(&mut stdout);
            if v["id"] == id && v.get("method").is_none() {
                return v;
            }
        }
    };
    ask("initialize", json!({"protocolVersion": 1}));
    let r = ask(
        "session/resume",
        json!({"sessionId": sid, "cwd": wd, "mcpServers": []}),
    );
    assert!(r.get("error").is_none(), "{r}");
    ask("session/prompt", prompt("third"));
    assert_eq!(read(&sid)["lines"].as_array().unwrap().len(), 6);
    assert!(!launch.join(".rung/sessions").exists());
    ask("session/delete", json!({"sessionId": sid}));
    assert!(!sess_file(&sid).exists(), "delete missed the session cwd");
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&tmp);
}
