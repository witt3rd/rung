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
            let (ctype, payload) = if stream {
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
                "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
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
