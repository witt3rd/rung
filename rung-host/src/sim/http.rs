//! A scripted provider on loopback: an HTTP/1.1 server on 127.0.0.1 that
//! answers each request through a script, in the OpenAI-compatible wire
//! shape a router documents (SSE completions with a usage chunk carrying
//! `cost` and the prompt-token details; a provider's 429 with the router's
//! provider metadata; the router's own 429 with `X-RateLimit-*`; a model
//! listing). It keeps every request it saw, for the slice-2 gates.
//!
//! Nothing here reaches past loopback, needs a key, or is a product path.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::gates::{HttpSeen, Served};

/// One request, as the script sees it.
#[derive(Debug)]
pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub body: &'a Value,
    /// Requests seen before this one.
    pub n: usize,
}

/// One answer.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub content_type: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// What a completion served (for the gate).
    pub served: Option<Served>,
    /// `provider` or `platform`, for a 429.
    pub refusal: Option<String>,
    pub reset_at: Option<i64>,
}

impl Reply {
    pub fn json(status: u16, body: &Value) -> Self {
        Self {
            status,
            content_type: "application/json".into(),
            headers: Vec::new(),
            body: body.to_string(),
            served: None,
            refusal: None,
            reset_at: None,
        }
    }

    /// A streamed completion: optional text, optional tool calls
    /// `(id, name, arguments)`, then the usage chunk and `[DONE]`.
    pub fn completion(
        provider: &str,
        content: Option<&str>,
        calls: &[(&str, &str, Value)],
        served: Served,
    ) -> Self {
        let mut out = String::from(": OPENROUTER PROCESSING\n\n");
        let chunk = |delta: Value, finish: Value| {
            json!({"id": "gen-1", "provider": provider, "model": served.model,
                   "object": "chat.completion.chunk", "created": 1_790_985_600,
                   "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]})
        };
        if let Some(text) = content {
            out.push_str(&format!(
                "data: {}\n\n",
                chunk(json!({"role": "assistant", "content": text}), Value::Null)
            ));
        }
        if !calls.is_empty() {
            let tc: Vec<Value> = calls
                .iter()
                .enumerate()
                .map(|(i, (id, name, args))| {
                    json!({"index": i, "id": id, "type": "function",
                           "function": {"name": name, "arguments": args.to_string()}})
                })
                .collect();
            out.push_str(&format!(
                "data: {}\n\n",
                chunk(json!({"role": "assistant", "tool_calls": tc}), Value::Null)
            ));
        }
        let finish = if calls.is_empty() {
            "stop"
        } else {
            "tool_calls"
        };
        out.push_str(&format!("data: {}\n\n", chunk(json!({}), json!(finish))));
        let usage = json!({"id": "gen-1", "provider": provider, "model": served.model,
            "object": "chat.completion.chunk", "choices": [],
            "usage": {"prompt_tokens": served.prompt, "completion_tokens": served.completion,
                      "total_tokens": served.prompt + served.completion, "cost": served.cost_usd,
                      "is_byok": false,
                      "prompt_tokens_details": {"cached_tokens": served.cached,
                                                "cache_write_tokens": served.cache_write},
                      "completion_tokens_details": {"reasoning_tokens": 0}}});
        out.push_str(&format!("data: {usage}\n\ndata: [DONE]\n\n"));
        Self {
            status: 200,
            content_type: "text/event-stream".into(),
            headers: Vec::new(),
            body: out,
            served: Some(served),
            refusal: None,
            reset_at: None,
        }
    }

    /// An upstream provider's 429, relayed by the router with its provider
    /// metadata.
    pub fn provider_429(provider: &str, retry_after_ms: u64) -> Self {
        let mut r = Self::json(
            429,
            &json!({"error": {"code": 429,
                "message": format!("{provider} is temporarily rate-limited upstream. Please retry shortly."),
                "metadata": {"provider_name": provider,
                             "raw": format!("{provider}: too many requests")}}}),
        );
        r.headers
            .push(("retry-after-ms".into(), retry_after_ms.to_string()));
        r.refusal = Some("provider".into());
        r
    }

    /// The router's own 429: an account quota, with `X-RateLimit-*`.
    pub fn platform_429(limit: u64, reset_at: i64, retry_after_ms: u64) -> Self {
        let mut r = Self::json(
            429,
            &json!({"error": {"code": 429,
                "message": "Rate limit exceeded: free-models-per-minute.",
                "metadata": {"headers": {"X-RateLimit-Limit": limit.to_string(),
                                         "X-RateLimit-Remaining": "0",
                                         "X-RateLimit-Reset": reset_at.to_string()}}}}),
        );
        r.headers.extend([
            ("x-ratelimit-limit".to_string(), limit.to_string()),
            ("x-ratelimit-remaining".to_string(), "0".to_string()),
            ("x-ratelimit-reset".to_string(), reset_at.to_string()),
            ("retry-after-ms".to_string(), retry_after_ms.to_string()),
        ]);
        r.refusal = Some("platform".into());
        r.reset_at = Some(reset_at);
        r
    }
}

type Script = Box<dyn FnMut(&Request<'_>) -> Reply + Send>;

/// The server. Dropping it stops the accept loop.
pub struct LoopbackProvider {
    /// The base URL, `http://127.0.0.1:PORT/api/v1`.
    pub url: String,
    seen: Arc<Mutex<Vec<HttpSeen>>>,
    stop: Arc<AtomicBool>,
    port: u16,
}

impl std::fmt::Debug for LoopbackProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopbackProvider")
            .field("url", &self.url)
            .finish()
    }
}

impl LoopbackProvider {
    /// Serve `script` on a fresh loopback port, one request at a time.
    pub fn start(script: impl FnMut(&Request<'_>) -> Reply + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().expect("addr").port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (s2, st2) = (seen.clone(), stop.clone());
        let mut script: Script = Box::new(script);
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if st2.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(mut sock) = conn else { continue };
                let loopback = sock.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false);
                let Some((method, path, body)) = read_request(&mut sock) else {
                    continue;
                };
                let n = s2.lock().expect("seen").len();
                let reply = script(&Request {
                    method: &method,
                    path: &path,
                    body: &body,
                    n,
                });
                s2.lock().expect("seen").push(HttpSeen {
                    method: method.clone(),
                    path: path.clone(),
                    loopback,
                    body: body.clone(),
                    status: reply.status,
                    served: reply.served.clone(),
                    refusal: reply.refusal.clone(),
                    reset_at: reply.reset_at,
                });
                let head: String = reply
                    .headers
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}\r\n"))
                    .collect();
                let resp = format!(
                    "HTTP/1.1 {} X\r\nContent-Type: {}\r\n{head}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                    reply.status,
                    reply.content_type,
                    reply.body.len(),
                    reply.body
                );
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.flush();
            }
        });
        Self {
            url: format!("http://127.0.0.1:{port}/api/v1"),
            seen,
            stop,
            port,
        }
    }

    /// Every request so far, in arrival order.
    pub fn seen(&self) -> Vec<HttpSeen> {
        self.seen.lock().expect("seen").clone()
    }
}

impl Drop for LoopbackProvider {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

/// Read one request: (method, path, JSON body or `Null`).
fn read_request(sock: &mut TcpStream) -> Option<(String, String, Value)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        let n = sock.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        let Some(at) = find(&buf, b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&buf[..at]).to_string();
        let len = head
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|v| v.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        if buf.len() < at + 4 + len {
            continue;
        }
        let mut first = head.lines().next()?.split(' ');
        let method = first.next()?.to_string();
        let path = first.next()?.to_string();
        let raw = &buf[at + 4..at + 4 + len];
        let body = if raw.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(raw).unwrap_or(Value::Null)
        };
        return Some((method, path, body));
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
