//! L12: in `Engine::turn` the caller owns the thread. The turn's transcript
//! starts with exactly the messages it was given, and what the turn added
//! comes back verbatim — a tool result longer than the CLI's history cap is
//! not shortened — so a caller that appends it and sends it again extends
//! the previous request byte for byte. Shortening is the caller's choice
//! (the CLI's session history; a host's rollover), never the engine's.
//!
//! Also: a listener set on the turn sees a refused attempt through the
//! per-call recorder.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use rung_agent_core::catalog::{Kind, Scope};
use rung_agent_core::config::{TurnCheckBackend, TurnCheckSettings};
use rung_agent_core::engine::{Engine, EngineSpec, Event, EventSink, TurnCtl};
use rung_std::agent::Thread;
use rung_std::llm::{
    ChatMessage, HttpFailure, LlmConfig, MessageContent, MessageContentBlock, Protocol,
    StreamEvent, StreamListener,
};
use serde_json::{Value, json};

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rung-engine-thread-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn read_body(sock: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 65536];
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

/// Each request gets the next reply, as JSON (the turns here do not
/// stream unless a listener is set; then as one SSE chunk). `__status`
/// answers with that status, `__headers` and `__body`.
fn mock_llm(replies: Vec<Value>) -> (String, Receiver<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for reply in replies {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let Some(body) = read_body(&mut sock) else {
                continue;
            };
            let body: Value = serde_json::from_str(&body).unwrap();
            let stream = body["stream"] == true;
            let _ = tx.send(body);
            let (status, ctype, payload) = if let Some(code) = reply["__status"].as_u64() {
                (code, "application/json", reply["__body"].to_string())
            } else if stream {
                let chunk = json!({"id": "c", "model": "served-model",
                    "choices": [{"delta": reply["choices"][0]["message"], "finish_reason": reply["choices"][0]["finish_reason"]}]});
                (
                    200,
                    "text/event-stream",
                    format!("data: {chunk}\n\ndata: [DONE]\n\n"),
                )
            } else {
                (200, "application/json", reply.to_string())
            };
            let headers: String = reply["__headers"]
                .as_object()
                .map(|h| {
                    h.iter()
                        .map(|(k, v)| format!("{k}: {}\r\n", v.as_str().unwrap()))
                        .collect()
                })
                .unwrap_or_default();
            let resp = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}/v1"), rx)
}

fn text_reply(text: &str) -> Value {
    json!({"id": "c", "model": "served-model", "choices": [{"message": {"content": text}, "finish_reason": "stop"}]})
}

fn call_reply(name: &str, args: Value) -> Value {
    json!({"id": "c", "model": "served-model", "choices": [{"message": {"content": null, "tool_calls": [
        {"id": "call_1", "type": "function", "function": {"name": name, "arguments": args.to_string()}}
    ]}, "finish_reason": "tool_calls"}]})
}

fn spec(url: &str, tools: &str, workspace: &std::path::Path) -> EngineSpec {
    let llm = LlmConfig {
        base_url: url.into(),
        api_key: "k".into(),
        model: "spec-model".into(),
        timeout_secs: 10,
        protocol: Protocol::OpenAiChat,
        max_tokens: 0,
        ..rung_agent_core::config::dummy()
    };
    EngineSpec {
        llm,
        kind: Kind::Implement,
        scope: Scope::parse(tools).unwrap(),
        max_iterations: None,
        turn_check: TurnCheckSettings {
            backend: TurnCheckBackend::Off,
            base_url: "http://127.0.0.1:9/api/v1".into(),
            model: "judge".into(),
            api_key_env: "UNUSED_JUDGE_KEY".into(),
            timeout_secs: 1,
        },
        tool_images: false,
        mcp: Vec::new(),
        workspace: workspace.to_path_buf(),
    }
}

struct Quiet;

impl EventSink for Quiet {
    fn event(&self, _: &Event<'_>) {}
}

fn ctl() -> TurnCtl {
    TurnCtl {
        sink: Arc::new(Quiet),
        ..TurnCtl::default()
    }
}

fn tool_result_text(m: &ChatMessage) -> Option<String> {
    match &m.content {
        MessageContent::Blocks(bs) => bs.iter().find_map(|b| match b {
            MessageContentBlock::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        }),
        MessageContent::Text(_) => None,
    }
}

#[test]
fn the_caller_owns_the_thread_and_a_long_tool_result_comes_back_verbatim() {
    let tmp = tempdir("verbatim");
    // Longer than the CLI's 4,000-char history cap, inside the live
    // tool-output cap.
    let long: String = (0..180)
        .map(|i| format!("line {i:04} of the long file\n"))
        .collect();
    let file = tmp.join("long.txt");
    std::fs::write(&file, &long).unwrap();
    let (url, bodies) = mock_llm(vec![
        call_reply("read_file", json!({"path": file.to_string_lossy()})),
        text_reply("read it"),
        text_reply("second turn"),
    ]);
    let engine = Engine::new(spec(&url, "read", &tmp)).unwrap();
    let given = Thread {
        system_prompt: "stable".into(),
        messages: vec![
            ChatMessage::user("slow"),
            ChatMessage::user("read long.txt"),
        ],
    };
    let sent = given.messages.len();
    let report = engine.turn(given.clone(), ctl());
    let done = report.outcome.expect("the turn answers");
    let t = &done.result.transcript;
    assert_eq!(
        &t[..sent],
        &given.messages[..],
        "nothing given is rewritten"
    );
    let added = &t[sent..];
    let result = added
        .iter()
        .find_map(tool_result_text)
        .expect("the tool result is in the turn");
    assert!(
        result.len() > 5_000,
        "not shortened: {} chars",
        result.len()
    );
    assert!(result.contains("line 0179 of the long file"), "{result}");
    assert!(!result.contains("shortened in history"));

    // The caller appends the turn as it came back and asks again: the second
    // request's messages extend the first turn's last request exactly.
    let _first = bodies.recv().unwrap();
    let last_of_turn = bodies.recv().unwrap();
    let mut next = given.clone();
    next.messages.extend(added.iter().cloned());
    next.messages.push(ChatMessage::user("next header"));
    let _ = engine.turn(next, ctl());
    let second_turn = bodies.recv().unwrap();
    let a = last_of_turn["messages"].as_array().unwrap();
    let b = second_turn["messages"].as_array().unwrap();
    assert!(b.len() > a.len());
    assert_eq!(&b[..a.len()], &a[..], "a byte-for-byte prefix extension");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[derive(Default)]
struct Failures(Mutex<Vec<HttpFailure>>);

impl StreamListener for Failures {
    fn on_event(&self, _: StreamEvent) {}
    fn on_http_failure(&self, f: &HttpFailure) {
        self.0.lock().unwrap().push(f.clone());
    }
}

#[test]
fn a_turns_listener_sees_each_refused_attempt() {
    let tmp = tempdir("refused");
    let limited = json!({"__status": 429,
        "__headers": {"retry-after-ms": "20", "x-ratelimit-reset": "1790990000000"},
        "__body": {"error": {"message": "slow down"}}});
    let (url, _) = mock_llm(vec![limited.clone(), limited.clone(), limited]);
    let engine = Engine::new(spec(&url, "none", &tmp)).unwrap();
    let seen = Arc::new(Failures::default());
    let report = engine.turn(
        Thread {
            system_prompt: "s".into(),
            messages: vec![ChatMessage::user("hi")],
        },
        TurnCtl {
            stream_listener: Some(seen.clone()),
            ..ctl()
        },
    );
    assert!(report.failure.is_some());
    let f = seen.0.lock().unwrap();
    assert_eq!(f.len(), 3, "one per attempt: {f:?}");
    assert!(f.iter().all(|x| x.status == 429));
    assert!(
        f[0].headers
            .contains(&("x-ratelimit-reset".into(), "1790990000000".into()))
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn a_turn_may_name_its_model_and_session_over_the_specs() {
    let tmp = tempdir("override");
    let (url, bodies) = mock_llm(vec![text_reply("a"), text_reply("b")]);
    let engine = Engine::new(spec(&url, "none", &tmp)).unwrap();
    let thread = || Thread {
        system_prompt: "s".into(),
        messages: vec![ChatMessage::user("hi")],
    };
    let _ = engine.turn(thread(), ctl());
    let _ = engine.turn(
        thread(),
        TurnCtl {
            model: Some("ladder/rung-1".into()),
            session_id: Some("epoch-3".into()),
            ..ctl()
        },
    );
    let first = bodies.recv().unwrap();
    assert_eq!(first["model"], "spec-model");
    assert!(first.get("session_id").is_none(), "unset sends nothing");
    let second = bodies.recv().unwrap();
    assert_eq!(second["model"], "ladder/rung-1");
    assert_eq!(second["session_id"], "epoch-3");
    let _ = std::fs::remove_dir_all(&tmp);
}
