//! `Engine::turn` driven directly, against a local mock of an
//! OpenAI-compatible model: a caller-assembled thread in, a `TurnReport` out,
//! no session store, diagnostics to the caller's sink, the tool gate, the
//! per-call record and typed provider failures.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use rung_agent_core::catalog::{Kind, Scope};
use rung_agent_core::config::{TurnCheckBackend, TurnCheckSettings};
use rung_agent_core::engine::{
    Engine, EngineSpec, Event, EventSink, ProviderClass, ProviderFailure, ToolGate, TurnCtl,
    TurnReport,
};
use rung_agent_core::run::Status;
use rung_std::agent::{FailureKind, Thread};
use rung_std::llm::{ChatMessage, LlmConfig, Protocol, StreamEvent, StreamListener};
use serde_json::{Value, json};

fn tempdir(tag: &str) -> rung_testkit::TempDir {
    rung_testkit::TempDir::new(&format!("engine-{tag}"))
}

fn read_body(sock: &mut TcpStream) -> Option<String> {
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

/// OpenAI-compatible mock: each request gets the next reply (SSE when the
/// request streams). `__status`, `__headers` and `__body` answer with that
/// HTTP status instead of a completion. Every request body is sent on `rx`.
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
                let mut delta = reply["choices"][0]["message"].clone();
                if let Some(calls) = delta.get_mut("tool_calls").and_then(|c| c.as_array_mut()) {
                    for (i, c) in calls.iter_mut().enumerate() {
                        c["index"] = json!(i);
                    }
                }
                let chunk = json!({"id": "c", "model": "served-model",
                    "choices": [{"delta": delta, "finish_reason": reply["choices"][0]["finish_reason"]}]});
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

fn ask(text: &str) -> Thread {
    Thread {
        system_prompt: "You are a test.".into(),
        messages: vec![ChatMessage::user(text)],
    }
}

/// Keeps every event the turn emits.
#[derive(Default)]
struct Keep(Mutex<Vec<(String, String, String)>>);

impl EventSink for Keep {
    fn event(&self, e: &Event<'_>) {
        self.0
            .lock()
            .unwrap()
            .push((e.source.into(), e.kind.into(), e.line.into()));
    }
}

impl Keep {
    fn kinds(&self) -> Vec<String> {
        self.0.lock().unwrap().iter().map(|e| e.1.clone()).collect()
    }
}

fn ctl(sink: &Arc<Keep>) -> TurnCtl {
    TurnCtl {
        sink: sink.clone(),
        request: "the ask".into(),
        ..TurnCtl::default()
    }
}

fn done(report: TurnReport) -> rung_agent_core::engine::TurnDone {
    match report.outcome {
        Ok(d) => d,
        Err(f) => panic!("turn stopped: {} ({:?})", f.reason, f.kind),
    }
}

#[test]
fn a_turn_runs_on_the_callers_thread_and_touches_no_session_store() {
    let tmp = tempdir("turn");
    let file = tmp.join("a.txt");
    std::fs::write(&file, "alpha\n").unwrap();
    let (url, bodies) = mock_llm(vec![
        call_reply("read_file", json!({"path": file.to_string_lossy()})),
        text_reply("a.txt says alpha"),
    ]);
    let engine = Engine::new(spec(&url, "read", &tmp)).unwrap();
    let sink = Arc::new(Keep::default());
    let report = engine.turn(ask("what is in a.txt?"), ctl(&sink));
    assert!(report.failure.is_none());
    assert!(report.calls.is_empty(), "no stream, no per-call record");
    let d = done(report);
    assert!(matches!(d.status, Status::Completed(_)), "{:?}", d.status);
    assert_eq!(d.result.final_response, "a.txt says alpha");
    assert_eq!(d.api_calls, 2);
    assert!(d.turn_check.is_none());

    let first = bodies.recv().unwrap();
    assert_eq!(first["model"], "spec-model", "the spec's values, not env");
    assert_eq!(first["messages"][0]["content"], "You are a test.");
    assert_eq!(first["messages"][1]["content"], "what is in a.txt?");
    let second = bodies.recv().unwrap();
    assert!(second.to_string().contains("alpha"), "{second}");

    assert_eq!(
        sink.kinds(),
        [
            "llm.call",
            "tool.call",
            "tool.result",
            "turn.iterate",
            "llm.call",
            "turn.end"
        ]
    );
    assert!(sink.0.lock().unwrap().iter().all(|e| e.0 == "rung-std"));
    assert!(!tmp.join(".rung").join("sessions").exists());
}

#[test]
fn an_engine_keeps_across_turns_and_the_caller_owns_the_thread() {
    let tmp = tempdir("turns");
    let (url, bodies) = mock_llm(vec![text_reply("one"), text_reply("two")]);
    let engine = Engine::new(spec(&url, "none", &tmp)).unwrap();
    let sink = Arc::new(Keep::default());
    let first = done(engine.turn(ask("first"), ctl(&sink)));
    let mut thread = ask("first");
    thread
        .messages
        .push(ChatMessage::assistant(first.result.final_response));
    thread.messages.push(ChatMessage::user("second"));
    let second = done(engine.turn(thread, ctl(&sink)));
    assert_eq!(second.result.final_response, "two");
    let _ = bodies.recv().unwrap();
    let body = bodies.recv().unwrap();
    let roles: Vec<&str> = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["system", "user", "assistant", "user"]);
}

#[test]
fn a_disabled_tool_stays_declared_is_refused_and_never_runs() {
    let tmp = tempdir("gate");
    let target = tmp.join("written.txt");
    let (url, bodies) = mock_llm(vec![
        call_reply(
            "write_file",
            json!({"path": target.to_string_lossy(), "content": "x"}),
        ),
        text_reply("could not write"),
    ]);
    let engine = Engine::new(spec(&url, "write", &tmp)).unwrap();
    let sink = Arc::new(Keep::default());
    let report = engine.turn(
        ask("write it"),
        TurnCtl {
            gate: ToolGate::Disabled(BTreeSet::from(["write_file".to_string()])),
            ..ctl(&sink)
        },
    );
    let d = done(report);
    assert_eq!(d.result.final_response, "could not write");
    assert!(!target.exists(), "a disabled tool must not run");
    let first = bodies.recv().unwrap();
    assert!(
        first["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["function"]["name"] == "write_file"),
        "still declared: {first}"
    );
    let second = bodies.recv().unwrap().to_string();
    assert!(second.contains("refused"), "{second}");
    assert!(second.contains("disabled for this turn"), "{second}");
}

struct Quiet;

impl StreamListener for Quiet {
    fn on_event(&self, _: StreamEvent) {}
}

#[test]
fn a_streaming_turn_records_each_call_as_served() {
    let tmp = tempdir("calls");
    let file = tmp.join("b.txt");
    std::fs::write(&file, "beta\n").unwrap();
    let (url, _bodies) = mock_llm(vec![
        call_reply("read_file", json!({"path": file.to_string_lossy()})),
        text_reply("beta"),
    ]);
    let engine = Engine::new(spec(&url, "read", &tmp)).unwrap();
    let sink = Arc::new(Keep::default());
    let report = engine.turn(
        ask("read b.txt"),
        TurnCtl {
            stream_listener: Some(Arc::new(Quiet)),
            ..ctl(&sink)
        },
    );
    assert_eq!(report.calls.len(), 2, "{:?}", report.calls);
    assert!(report.calls.iter().all(|c| c.model == "served-model"));
    assert_eq!(done(report).result.final_response, "beta");
}

#[test]
fn an_auth_failure_is_typed_on_the_report() {
    let tmp = tempdir("auth");
    let (url, _) = mock_llm(vec![
        json!({"__status": 401, "__body": {"error": {"message": "bad key"}}}),
    ]);
    let engine = Engine::new(spec(&url, "none", &tmp)).unwrap();
    let sink = Arc::new(Keep::default());
    let report = engine.turn(ask("hi"), ctl(&sink));
    assert_eq!(
        report.failure,
        Some(ProviderFailure {
            class: ProviderClass::Auth,
            retry_after_ms: None,
        })
    );
    let f = report.outcome.unwrap_err();
    assert_eq!(f.kind, FailureKind::Auth);
}

#[test]
fn a_rate_limit_is_typed_with_the_wait_the_provider_asked_for() {
    let tmp = tempdir("ratelimit");
    let limited = json!({"__status": 429, "__headers": {"retry-after-ms": "40"},
        "__body": {"error": {"message": "slow down"}}});
    let (url, _) = mock_llm(vec![limited.clone(), limited.clone(), limited]);
    let engine = Engine::new(spec(&url, "none", &tmp)).unwrap();
    let sink = Arc::new(Keep::default());
    let report = engine.turn(ask("hi"), ctl(&sink));
    assert_eq!(
        report.failure,
        Some(ProviderFailure {
            class: ProviderClass::RateLimit,
            retry_after_ms: Some(40),
        })
    );
    assert!(sink.kinds().contains(&"llm.retry".to_string()));
}

#[test]
fn a_turn_with_no_sink_set_reports_to_standard_error() {
    let tmp = tempdir("stderr");
    let (url, _) = mock_llm(vec![text_reply("ok")]);
    let engine = Engine::new(spec(&url, "none", &tmp)).unwrap();
    let report = engine.turn(ask("hi"), TurnCtl::default());
    assert_eq!(done(report).result.final_response, "ok");
}

/// A stdio MCP server with one tool, `ping`, that exits after its first
/// `tools/call`, as a provider process that died between turns would.
const ONE_CALL_SERVER: &str = r#"
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if "id" not in msg:
        continue
    method = msg["method"]
    if method == "initialize":
        result = {"protocolVersion": msg["params"]["protocolVersion"],
                  "capabilities": {"tools": {}},
                  "serverInfo": {"name": "one-call", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": [{"name": "ping", "description": "Ping.",
                             "inputSchema": {"type": "object"}}]}
    else:
        result = {"content": [{"type": "text", "text": "pong"}]}
    print(json.dumps({"jsonrpc": "2.0", "id": msg["id"], "result": result}), flush=True)
    if method == "tools/call":
        sys.exit(0)
"#;

#[test]
fn the_mcp_roster_lives_across_turns_and_reconnects_when_a_server_is_gone() {
    let tmp = tempdir("mcp");
    let (url, bodies) = mock_llm(vec![
        call_reply("ping", json!({})),
        text_reply("pinged once"),
        call_reply("ping", json!({})),
        text_reply("pinged twice"),
    ]);
    let mut s = spec(&url, "none", &tmp);
    s.mcp = vec![rung_agent_core::mcp::McpSpec::Stdio {
        name: "one-call".into(),
        command: "python3".into(),
        args: vec!["-c".into(), ONE_CALL_SERVER.into()],
        env: Vec::new(),
    }];
    let engine = Engine::new(s).unwrap();
    let sink = Arc::new(Keep::default());
    let first = done(engine.turn(ask("ping it"), ctl(&sink)));
    assert_eq!(first.result.final_response, "pinged once");
    assert!(!sink.kinds().contains(&"mcp.reconnect".to_string()));
    // Give the server its moment to exit after the call it answered.
    std::thread::sleep(std::time::Duration::from_millis(300));
    let second = done(engine.turn(ask("ping it again"), ctl(&sink)));
    assert_eq!(second.result.final_response, "pinged twice");
    assert!(
        sink.kinds().contains(&"mcp.reconnect".to_string()),
        "{:?}",
        sink.kinds()
    );
    let results: Vec<String> = (0..4).map(|_| bodies.recv().unwrap().to_string()).collect();
    assert!(results[1].contains("pong"), "{}", results[1]);
    assert!(results[3].contains("pong"), "{}", results[3]);
}
