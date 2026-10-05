use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rung-agent"))
}

#[test]
fn help_exits_zero() {
    let out = bin().arg("--help").output().unwrap();
    assert!(out.status.success(), "{:?}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--toolset"), "{text}");
    assert!(text.contains("--type"), "{text}");
    assert!(text.contains("--tools"), "{text}");
    assert!(text.contains("--isolation"), "{text}");
    assert!(text.contains("--background"), "{text}");
    assert!(text.contains("--acp"), "{text}");
    assert!(text.contains("--acp-http"), "{text}");
}

#[test]
fn missing_prompt_is_usage() {
    let out = bin().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn poll_missing_session() {
    let tmp = tempfile();
    let out = bin()
        .current_dir(&tmp)
        .args(["--task-id", "no-such-id"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{:?}", out);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no session"), "{err}");
}

#[test]
fn poll_completed_session() {
    let tmp = tempfile();
    let dir = tmp.join(".rung").join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("abc-1.json"),
        r#"{
  "id": "abc-1",
  "kind": "explore",
  "status": "completed",
  "cwd": ".",
  "lines": [
    {"role": "user", "text": "look"},
    {"role": "assistant", "text": "found it"}
  ]
}"#,
    )
    .unwrap();
    let out = bin()
        .current_dir(&tmp)
        .args(["--task-id", "abc-1"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("status=completed"), "{text}");
    assert!(text.contains("found it"), "{text}");
}

#[test]
fn poll_completed_session_json() {
    let tmp = tempfile();
    let dir = tmp.join(".rung").join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("abc-1.json"),
        r#"{
  "id": "abc-1",
  "kind": "implement",
  "status": "completed",
  "cwd": ".",
  "lines": [
    {"role": "user", "text": "look"},
    {"role": "assistant", "text": "found it"}
  ]
}"#,
    )
    .unwrap();
    let out = bin()
        .current_dir(&tmp)
        .args(["--json", "--task-id", "abc-1"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let text = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["task_id"], "abc-1");
    assert_eq!(parsed["status"], "completed");
    assert_eq!(parsed["text"], "found it");
}

#[test]
fn background_unreachable_endpoint_records_error() {
    let tmp = tempfile();
    let out = bin()
        .current_dir(&tmp)
        .env("RUNG_CONFIG", tmp.join("no-such-config.yaml"))
        .env_remove("RUNG_API_KEY")
        .env_remove("XAI_API_KEY")
        .env("RUNG_BASE_URL", "http://127.0.0.1:9/v1")
        .env("RUNG_MODEL", "dummy")
        .env("RUNG_TIMEOUT_SECS", "1")
        .args(["--background", "--type", "explore", "look around"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("task_id="), "{text}");
    let id = text
        .lines()
        .find_map(|l| l.strip_prefix("task_id="))
        .expect("task_id line");
    let sess = tmp
        .join(".rung")
        .join("sessions")
        .join(format!("{id}.json"));
    let mut body = String::new();
    for _ in 0..80 {
        if let Ok(s) = std::fs::read_to_string(&sess) {
            body = s;
            if body.contains("\"status\": \"error\"") || body.contains("\"status\":\"error\"") {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(body.contains("error"), "session never failed: {body}");
}

fn tempfile() -> rung_testkit::TempDir {
    rung_testkit::TempDir::new("agent-cli")
}

/// OpenAI-compatible mock for a long job: `calls` tool calls, each with new
/// input (a poll that moves on), then a final answer. SSE when asked.
fn long_job_llm(calls: usize) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for i in 0..=calls {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 65536];
            let body = loop {
                let n = sock.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break String::new();
                }
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).into_owned();
                let Some(at) = text.find("\r\n\r\n") else {
                    continue;
                };
                let len = text[..at]
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if buf.len() >= at + 4 + len {
                    break String::from_utf8_lossy(&buf[at + 4..at + 4 + len]).into_owned();
                }
            };
            let stream =
                serde_json::from_str::<serde_json::Value>(&body).is_ok_and(|b| b["stream"] == true);
            let (message, finish) = if i < calls {
                let args = format!("{{\"frame\": {i}}}");
                (
                    serde_json::json!({"content": null, "tool_calls": [{"index": 0, "id": format!("c{i}"),
                        "type": "function", "function": {"name": "poll", "arguments": args}}]}),
                    "tool_calls",
                )
            } else {
                (serde_json::json!({"content": "rendered"}), "stop")
            };
            let (ctype, payload) = if stream {
                let chunk = serde_json::json!({"id": "c", "model": "m",
                    "choices": [{"delta": message, "finish_reason": finish}]});
                (
                    "text/event-stream",
                    format!("data: {chunk}\n\ndata: [DONE]\n\n"),
                )
            } else {
                let reply = serde_json::json!({"id": "c", "model": "m",
                    "choices": [{"message": message, "finish_reason": finish}]});
                ("application/json", reply.to_string())
            };
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        }
    });
    format!("http://127.0.0.1:{port}/v1")
}

/// A job of 40 tool calls: the toolset default (32 model calls) stops it;
/// the host's `--max-iterations` raises the cap, and `0` removes it.
#[test]
fn the_host_sets_the_per_prompt_call_cap() {
    for (cap, finishes) in [(None, false), (Some("64"), true), (Some("0"), true)] {
        let tmp = tempfile();
        let mut c = bin();
        c.current_dir(&tmp)
            .env("HOME", &tmp)
            .env("RUNG_CONFIG", tmp.join("none.yaml"))
            .env("RUNG_HOME", &tmp)
            .env("RUNG_BASE_URL", long_job_llm(40))
            .env("RUNG_MODEL", "m")
            .env("RUNG_API_KEY", "k")
            .env("RUNG_PROTOCOL", "openai")
            .env_remove("RUNG_SYSTEM_PROMPT_FILE")
            .env_remove("RUNG_TURN_CHECK");
        if let Some(n) = cap {
            c.args(["--max-iterations", n]);
        }
        let out = c
            .args(["--tools", "none", "--json", "render the piece"])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        if finishes {
            assert!(out.status.success(), "cap {cap:?}: {stderr}");
            let o: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
            assert_eq!(o["status"], "completed", "cap {cap:?}");
            assert_eq!(o["text"], "rendered", "cap {cap:?}");
            assert_eq!(o["api_calls"], 41, "cap {cap:?}");
        } else {
            assert!(!out.status.success(), "cap {cap:?}: {stdout}");
            assert!(
                stderr.contains("max iterations (32)"),
                "cap {cap:?}: {stderr}"
            );
        }
    }
}

#[test]
fn memory_scope_ls_rm_drop() {
    use rung_memory::{Record, RecordId, Scope};
    let tmp = tempfile();
    let scope = Scope::new("s1");
    let store = rung_memory::baseline::Baseline::new(&*tmp);
    let rec = |id: &str, at: &str| Record {
        id: RecordId::new(id),
        scope: scope.clone(),
        text: format!("note {id}"),
        observed_at: Some(at.into()),
        attrs: Default::default(),
    };
    std::fs::create_dir_all(&*tmp).unwrap();
    let lines: String = [
        rec("a", "2026-01-01T00:00:00Z"),
        rec("b", "2026-02-01T00:00:00Z"),
    ]
    .iter()
    .map(|r| serde_json::to_string(r).unwrap() + "\n")
    .collect();
    std::fs::write(store.file(&scope), lines).unwrap();
    let run = |args: &[&str]| {
        bin()
            .env("RUNG_MEMORY_SCOPE", "s1")
            .env("RUNG_MEMORY_DIR", &*tmp)
            .arg("--memory-scope")
            .args(args)
            .output()
            .unwrap()
    };
    let o = run(&["ls"]);
    let t = String::from_utf8_lossy(&o.stdout);
    assert!(
        t.contains("records: 2") && t.contains("newest: 2026-02-01"),
        "{t}"
    );
    assert!(run(&["rm", "a"]).status.success());
    assert!(String::from_utf8_lossy(&run(&["ls"]).stdout).contains("records: 1"));
    assert_eq!(run(&["rm", "zzz"]).status.code(), Some(1));
    assert!(run(&["drop"]).status.success());
    assert!(String::from_utf8_lossy(&run(&["ls"]).stdout).contains("records: 0"));
}
