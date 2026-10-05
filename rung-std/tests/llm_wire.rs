//! The request-level wire seams a long-lived host needs, pinned on the
//! compiled bodies (`llm::prepare`) and on a loopback HTTP exchange:
//!
//! - unset, they change nothing: the body of a representative request is
//!   pinned field by field;
//! - `session_id` is lowered on the OpenAI-compatible wire and is a no-op on
//!   Anthropic;
//! - explicit cache breakpoints are lowered per protocol, at the end of the
//!   part they name, and nowhere else;
//! - `CachePolicy::Auto` marks the system text and the latest user message
//!   on a route that takes markers, and only when the caller placed none;
//!   with markers set aside each request is a byte-prefix of the next;
//! - a non-2xx answer is shown to the stream listener (status, rate-limit
//!   headers only, the body with the key redacted), without counting as
//!   observed output.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use rung_std::llm::{
    self, CacheBreakpoint, CachePolicy, ChatMessage, HttpFailure, LlmConfig, MessageContentBlock,
    Protocol, StreamEvent, StreamListener, ToolDefinition,
};
use serde_json::{Value, json};

fn cfg(protocol: Protocol) -> LlmConfig {
    LlmConfig {
        base_url: "http://127.0.0.1:9/v1".into(),
        api_key: "sk-test-secret-key".into(),
        model: "m".into(),
        timeout_secs: 5,
        idle_timeout_secs: None,
        max_tokens: 64,
        temperature: None,
        top_p: None,
        top_k: None,
        seed: None,
        stop: vec![],
        reasoning_level: None,
        structured_outputs: false,
        protocol,
        cache: CachePolicy::None,
        stream_listener: None,
        session_id: None,
        cache_breakpoints: Vec::new(),
    }
}

/// system · slow user message · a header · an assistant tool call · its result.
fn thread() -> Vec<ChatMessage> {
    vec![
        ChatMessage::system("stable"),
        ChatMessage::user("slow"),
        ChatMessage::user("header"),
        ChatMessage::assistant_with_blocks(vec![MessageContentBlock::ToolUse {
            id: "c1".into(),
            name: "note".into(),
            input: json!({"text": "x"}),
            cache: None,
        }]),
        ChatMessage::tool_result("c1", "ok"),
    ]
}

fn tools() -> Vec<ToolDefinition> {
    vec![ToolDefinition::new(
        "note",
        "Note.",
        json!({"type": "object"}),
    )]
}

fn body(c: &LlmConfig) -> Value {
    llm::prepare(c, &thread(), &tools()).unwrap().body
}

fn markers(v: &Value) -> usize {
    match v {
        Value::Object(m) => {
            usize::from(m.contains_key("cache_control")) + m.values().map(markers).sum::<usize>()
        }
        Value::Array(a) => a.iter().map(markers).sum(),
        _ => 0,
    }
}

#[test]
fn unset_the_openai_body_is_unchanged() {
    let b = body(&cfg(Protocol::OpenAiChat));
    assert_eq!(
        b,
        json!({
            "model": "m",
            "max_tokens": 64,
            "messages": [
                {"role": "system", "content": "stable"},
                {"role": "user", "content": "slow"},
                {"role": "user", "content": "header"},
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": "c1", "type": "function", "function": {"name": "note", "arguments": "{\"text\":\"x\"}"}}
                ]},
                {"role": "tool", "tool_call_id": "c1", "content": "ok"}
            ],
            "tools": [{"type": "function", "function": {"name": "note", "description": "Note.", "parameters": {"type": "object"}}}]
        })
    );
}

#[test]
fn unset_the_anthropic_body_places_no_marker_under_policy_none() {
    let b = body(&cfg(Protocol::AnthropicMessages));
    assert_eq!(markers(&b), 0, "{b}");
    assert!(b.get("session_id").is_none());
    assert_eq!(b["system"], json!([{"type": "text", "text": "stable"}]));
    assert_eq!(b["messages"][0], json!({"role": "user", "content": "slow"}));
}

#[test]
fn the_session_id_rides_the_openai_wire_and_is_a_no_op_on_anthropic() {
    let mut c = cfg(Protocol::OpenAiChat);
    c.session_id = Some("epoch-7".into());
    let b = body(&c);
    assert_eq!(b["session_id"], "epoch-7");
    let mut plain = b.clone();
    plain.as_object_mut().unwrap().remove("session_id");
    assert_eq!(
        plain,
        body(&cfg(Protocol::OpenAiChat)),
        "nothing else moves"
    );

    let mut a = cfg(Protocol::AnthropicMessages);
    a.session_id = Some("epoch-7".into());
    assert_eq!(body(&a), body(&cfg(Protocol::AnthropicMessages)));
}

#[test]
fn openai_breakpoints_mark_the_end_of_the_parts_they_name_and_nothing_else() {
    let mut c = cfg(Protocol::OpenAiChat);
    c.cache_breakpoints = vec![CacheBreakpoint::System, CacheBreakpoint::Message(0)];
    let b = body(&c);
    let m = &b["messages"];
    assert_eq!(
        m[0]["content"],
        json!([{"type": "text", "text": "stable", "cache_control": {"type": "ephemeral"}}])
    );
    assert_eq!(
        m[1]["content"],
        json!([{"type": "text", "text": "slow", "cache_control": {"type": "ephemeral"}}])
    );
    assert_eq!(markers(&b), 2, "{b}");
    let plain = body(&cfg(Protocol::OpenAiChat));
    for i in 2..5 {
        assert_eq!(m[i], plain["messages"][i], "message {i} untouched");
    }
    assert_eq!(b["tools"], plain["tools"]);
}

#[test]
fn an_openai_breakpoint_on_a_bare_tool_call_or_past_the_end_is_a_no_op() {
    let mut c = cfg(Protocol::OpenAiChat);
    // Message(2) is the assistant's bare tool call (no content); Message(9)
    // is past the end.
    c.cache_breakpoints = vec![CacheBreakpoint::Message(2), CacheBreakpoint::Message(9)];
    assert_eq!(body(&c), body(&cfg(Protocol::OpenAiChat)));
    // Message(3) is the tool result: its (only) wire message is marked.
    c.cache_breakpoints = vec![CacheBreakpoint::Message(3)];
    let b = body(&c);
    assert_eq!(
        b["messages"][4]["content"],
        json!([{"type": "text", "text": "ok", "cache_control": {"type": "ephemeral"}}])
    );
    assert_eq!(markers(&b), 1);
}

#[test]
fn anthropic_breakpoints_are_stamped_whatever_the_policy_and_keep_the_cap() {
    let mut c = cfg(Protocol::AnthropicMessages);
    c.cache_breakpoints = vec![CacheBreakpoint::System, CacheBreakpoint::Message(0)];
    let b = body(&c);
    assert_eq!(markers(&b), 2, "{b}");
    assert_eq!(
        b["system"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
    assert_eq!(
        b["messages"][0]["content"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );

    // With the automatic placement too: last tool, the system (already
    // marked), the slow message, and the latest user message — four, the cap.
    c.cache = CachePolicy::Auto;
    let b = body(&c);
    assert_eq!(markers(&b), 4, "{b}");
    assert_eq!(b["tools"][0]["cache_control"], json!({"type": "ephemeral"}));
    let last = b["messages"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(
        last["content"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
}

// ─── The listener sees a refused attempt ─────────────────────────────────────

#[derive(Default)]
struct Seen {
    failures: Mutex<Vec<HttpFailure>>,
    events: Mutex<usize>,
}

impl StreamListener for Seen {
    fn on_event(&self, _: StreamEvent) {
        *self.events.lock().unwrap() += 1;
    }
    fn on_http_failure(&self, f: &HttpFailure) {
        self.failures.lock().unwrap().push(f.clone());
    }
}

/// Answer one request with `status`, `headers` and `body`.
fn serve_once(status: u16, headers: &[(&str, &str)], body: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let head: String = headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}\r\n"))
        .collect();
    let body = body.to_string();
    std::thread::spawn(move || {
        let Ok((mut sock, _)) = listener.accept() else {
            return;
        };
        let mut buf = [0u8; 65536];
        let mut got = Vec::new();
        loop {
            let n = sock.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&got);
            if let Some(at) = text.find("\r\n\r\n") {
                let len = text[..at]
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if got.len() >= at + 4 + len {
                    break;
                }
            }
        }
        let resp = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n{head}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = sock.write_all(resp.as_bytes());
    });
    format!("http://127.0.0.1:{port}/v1")
}

fn refused(protocol: Protocol) -> (Arc<Seen>, llm::RawCallError) {
    let url = serve_once(
        429,
        &[
            ("X-RateLimit-Limit", "20"),
            ("X-RateLimit-Remaining", "0"),
            ("X-RateLimit-Reset", "1790990000000"),
            ("Retry-After", "7"),
            ("X-Request-Id", "req-abc"),
        ],
        r#"{"error":{"message":"Rate limit exceeded for key sk-test-secret-key"}}"#,
    );
    let seen = Arc::new(Seen::default());
    let mut c = cfg(protocol);
    c.base_url = url;
    c.stream_listener = Some(seen.clone());
    let err = llm::raw_call(&c, &[ChatMessage::user("hi")], &[]).unwrap_err();
    (seen, err)
}

fn check(seen: &Seen, err: &llm::RawCallError) {
    let f = seen.failures.lock().unwrap();
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(f[0].status, 429);
    assert_eq!(
        f[0].headers,
        vec![
            ("x-ratelimit-limit".to_string(), "20".to_string()),
            ("x-ratelimit-remaining".to_string(), "0".to_string()),
            ("x-ratelimit-reset".to_string(), "1790990000000".to_string()),
            ("retry-after".to_string(), "7".to_string()),
        ]
    );
    assert!(f[0].body.contains("Rate limit exceeded"), "{}", f[0].body);
    assert!(
        !f[0].body.contains("sk-test-secret-key"),
        "the key is redacted"
    );
    assert_eq!(*seen.events.lock().unwrap(), 0);
    // The classification and the retry policy are what they were.
    assert!(
        matches!(
            err,
            llm::RawCallError::RateLimit {
                retry_after_ms: Some(7000),
                ..
            }
        ),
        "{err:?}"
    );
    assert!(
        err.is_retryable(),
        "a refused attempt is not observed output"
    );
}

#[test]
fn an_openai_refusal_is_shown_to_the_listener() {
    let (seen, err) = refused(Protocol::OpenAiChat);
    check(&seen, &err);
}

#[test]
fn an_anthropic_refusal_is_shown_to_the_listener() {
    let (seen, err) = refused(Protocol::AnthropicMessages);
    check(&seen, &err);
}

// ─── Automatic markers on the OpenAI-compatible wire ─────────────────────────

/// The body with every `cache_control` removed: a marker marks, it is not
/// content, so it is not part of the prefix a provider caches.
fn unmarked(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| k.as_str() != "cache_control")
                .map(|(k, x)| (k.clone(), unmarked(x)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(unmarked).collect()),
        other => other.clone(),
    }
}

fn openrouter_auto() -> LlmConfig {
    let mut c = cfg(Protocol::OpenAiChat);
    c.base_url = "https://openrouter.ai/api/v1".into();
    c.cache = CachePolicy::Auto;
    c
}

#[test]
fn auto_marks_the_system_text_and_the_latest_user_message_on_a_route_that_takes_them() {
    let b = body(&openrouter_auto());
    let m = &b["messages"];
    let marked = |text: &str| json!([{"type": "text", "text": text, "cache_control": {"type": "ephemeral"}}]);
    assert_eq!(m[0]["content"], marked("stable"));
    assert_eq!(m[1]["content"], json!([{"type": "text", "text": "slow"}]));
    assert_eq!(m[2]["content"], marked("header"));
    assert_eq!(markers(&b), 2, "{b}");
    assert!(
        b.get("session_id").is_none(),
        "a session id is the caller's"
    );

    // A Claude model behind any OpenAI-compatible route is marked too.
    let mut c = cfg(Protocol::OpenAiChat);
    c.cache = CachePolicy::Auto;
    c.model = "anthropic/claude-sonnet-4.5".into();
    assert_eq!(markers(&body(&c)), 2);
}

#[test]
fn auto_marks_nothing_on_a_plain_route_or_beside_the_callers_breakpoints() {
    let mut c = cfg(Protocol::OpenAiChat);
    c.cache = CachePolicy::Auto;
    assert_eq!(body(&c), body(&cfg(Protocol::OpenAiChat)), "a plain route");

    let mut c = openrouter_auto();
    c.cache_breakpoints = vec![CacheBreakpoint::System, CacheBreakpoint::Message(0)];
    let mut explicit = cfg(Protocol::OpenAiChat);
    explicit.base_url = c.base_url.clone();
    explicit.cache_breakpoints = c.cache_breakpoints.clone();
    assert_eq!(body(&c), body(&explicit), "the caller's placement wins");

    let mut c = openrouter_auto();
    c.cache = CachePolicy::None;
    assert_eq!(markers(&body(&c)), 0);
}

/// The marker moves on to each new latest user message; with markers set
/// aside, the next turn's request extends this one's byte for byte.
#[test]
fn auto_marks_keep_each_request_a_byte_prefix_of_the_next() {
    let c = openrouter_auto();
    let turn1 = vec![
        ChatMessage::system("stable"),
        ChatMessage::user("first ask"),
    ];
    let mut turn2 = turn1.clone();
    turn2.push(ChatMessage::assistant("first answer"));
    turn2.push(ChatMessage::user("second ask"));
    let a = llm::prepare(&c, &turn1, &tools()).unwrap().body;
    let b = llm::prepare(&c, &turn2, &tools()).unwrap().body;
    assert_eq!(markers(&a), 2);
    assert_eq!(markers(&b), 2);
    let (a, b) = (unmarked(&a), unmarked(&b));
    assert_eq!(a["tools"], b["tools"]);
    let (am, bm) = (
        a["messages"].as_array().unwrap(),
        b["messages"].as_array().unwrap(),
    );
    for (i, m) in am.iter().enumerate() {
        assert_eq!(m.to_string(), bm[i].to_string(), "message {i}");
    }
}

#[test]
fn openrouter_is_recognised_by_its_host_only() {
    let mut c = cfg(Protocol::OpenAiChat);
    for (url, yes) in [
        ("https://openrouter.ai/api/v1", true),
        ("https://eu.openrouter.ai/api/v1", true),
        ("https://OpenRouter.ai:443/api/v1", true),
        ("http://127.0.0.1:9/v1", false),
        ("https://openrouter.ai.example.com/v1", false),
        ("https://example.com/openrouter.ai/v1", false),
    ] {
        c.base_url = url.into();
        assert_eq!(c.is_openrouter(), yes, "{url}");
    }
}
