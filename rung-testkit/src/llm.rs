//! One scripted OpenAI-compatible server for tests: [`mock_llm`].
//!
//! Each connection, in accept order, gets the next scripted reply. A reply is
//! a chat-completion JSON value, answered as JSON, or as one SSE chunk when the
//! request body asks to stream (tool calls get their `index`). Four reserved
//! keys shape the HTTP answer instead of a completion:
//!
//! - `__status` + `__body`: answer with that status and body (an HTTP error);
//! - `__headers`: extra response headers (an object of strings);
//! - `__delay_ms`: hold the answer back after the request was recorded.
//!
//! Connections are served on their own threads, so overlapping requests overlap.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// One request the mock received.
pub struct Request {
    /// The request line, e.g. `POST /v1/chat/completions HTTP/1.1`.
    pub line: String,
    /// The body as sent.
    pub raw: String,
    /// When the request was fully read.
    pub at: Instant,
}

impl Request {
    /// The body as JSON.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.raw).expect("request body is JSON")
    }
}

/// Serve `replies` on a loopback port and call `on_request` with each
/// request before it is answered. Returns the origin, `http://127.0.0.1:<port>`.
pub fn serve_llm(
    replies: Vec<Value>,
    on_request: impl Fn(&Request) + Send + Sync + 'static,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().expect("local addr").port();
    let on_request = Arc::new(on_request);
    std::thread::spawn(move || {
        for reply in replies {
            let Ok((sock, _)) = listener.accept() else {
                return;
            };
            let on_request = on_request.clone();
            std::thread::spawn(move || answer(sock, &reply, &*on_request));
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// [`serve_llm`] recording each request body on the returned channel.
pub fn mock_llm(replies: Vec<Value>) -> (String, Receiver<Value>) {
    let (tx, rx) = channel();
    let url = serve_llm(replies, move |r| {
        let _ = tx.send(r.json());
    });
    (url, rx)
}

fn read_request(sock: &mut TcpStream) -> Option<Request> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = sock.read(&mut chunk).ok()?;
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
                return Some(Request {
                    line: text.lines().next().unwrap_or_default().to_string(),
                    raw: String::from_utf8_lossy(&buf[at + 4..at + 4 + len]).to_string(),
                    at: Instant::now(),
                });
            }
        }
        if n == 0 {
            return None;
        }
    }
}

fn answer(mut sock: TcpStream, reply: &Value, on_request: &dyn Fn(&Request)) {
    let Some(request) = read_request(&mut sock) else {
        return;
    };
    let stream = request.json()["stream"] == true;
    on_request(&request);
    if let Some(ms) = reply["__delay_ms"].as_u64() {
        std::thread::sleep(Duration::from_millis(ms));
    }
    let (status, ctype, payload) = if let Some(code) = reply["__status"].as_u64() {
        (code, "application/json", reply["__body"].to_string())
    } else if stream {
        let mut delta = reply["choices"][0]["message"].clone();
        if let Some(calls) = delta.get_mut("tool_calls").and_then(|c| c.as_array_mut()) {
            for (i, c) in calls.iter_mut().enumerate() {
                c["index"] = json!(i);
            }
        }
        let chunk = json!({"id": reply["id"], "model": reply["model"],
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

#[cfg(test)]
mod tests {
    use super::*;

    fn post(url: &str, body: &Value) -> String {
        let host = url.trim_start_matches("http://");
        let body = body.to_string();
        let mut s = TcpStream::connect(host).unwrap();
        write!(
            s,
            "POST /v1 HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    fn text(t: &str) -> Value {
        json!({"id": "c", "model": "m", "choices": [{"message": {"content": t}, "finish_reason": "stop"}]})
    }

    #[test]
    fn replies_in_order_as_json_sse_and_status_and_records_bodies() {
        let (url, rx) = mock_llm(vec![
            text("one"),
            text("two"),
            json!({"__status": 429, "__headers": {"retry-after": "1"}, "__body": {"e": 1}}),
        ]);
        let a = post(&url, &json!({"n": 1}));
        assert!(
            a.starts_with("HTTP/1.1 200") && a.contains("application/json") && a.contains("one")
        );
        let b = post(&url, &json!({"n": 2, "stream": true}));
        assert!(b.contains("text/event-stream") && b.contains("\"delta\"") && b.contains("[DONE]"));
        let c = post(&url, &json!({"n": 3}));
        assert!(
            c.starts_with("HTTP/1.1 429") && c.contains("retry-after: 1"),
            "{c}"
        );
        let seen: Vec<Value> = rx.try_iter().collect();
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[1], json!({"n": 2, "stream": true}));
    }
}
