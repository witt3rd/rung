//! A scripted HTTP server for tests: [`Server`].
//!
//! The handler sees every request (method, path, headers, body) and returns
//! the answer, so a test can fake any provider endpoint, count the calls
//! that reach it, and change its answers as the test goes. Connections are
//! served on their own threads (overlapping requests overlap), and dropping
//! the [`Server`] stops the accept loop and joins it.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;

/// One request the server received.
#[derive(Debug, Clone)]
pub struct Req {
    pub method: String,
    /// The request target: path and query.
    pub path: String,
    /// Header names lower-cased.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Req {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The body as JSON.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).expect("request body is JSON")
    }

    /// The body as a URL-encoded form.
    pub fn form(&self) -> BTreeMap<String, String> {
        self.body
            .split('&')
            .filter(|p| !p.is_empty())
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap_or((p, ""));
                (decode(k), decode(v))
            })
            .collect()
    }
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The answer to one request.
#[derive(Debug, Clone)]
pub struct Resp {
    pub status: u16,
    pub content_type: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Resp {
    pub fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            content_type: "application/json".into(),
            headers: Vec::new(),
            body: body.to_string(),
        }
    }

    pub fn sse(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            content_type: "text/event-stream".into(),
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// A running server on a loopback port. Stops when dropped.
pub struct Server {
    url: String,
    seen: Arc<Mutex<Vec<Req>>>,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(handler: impl Fn(&Req) -> Resp + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        listener.set_nonblocking(true).expect("nonblocking");
        let url = format!("http://{}", listener.local_addr().expect("local addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handler = Arc::new(handler);
        let accept = {
            let (seen, stop) = (seen.clone(), stop.clone());
            std::thread::spawn(move || {
                let mut conns: Vec<JoinHandle<()>> = Vec::new();
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((sock, _)) => {
                            let (seen, handler) = (seen.clone(), handler.clone());
                            conns.push(std::thread::spawn(move || serve(sock, &seen, &*handler)));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(2)),
                    }
                }
                for c in conns {
                    let _ = c.join();
                }
            })
        };
        Self {
            url,
            seen,
            stop,
            accept: Some(accept),
        }
    }

    /// `http://127.0.0.1:<port>`, no trailing slash.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Every request so far, in arrival order.
    pub fn requests(&self) -> Vec<Req> {
        self.seen.lock().unwrap().clone()
    }

    /// How many requests had this method and a path starting with `prefix`.
    pub fn count(&self, method: &str, prefix: &str) -> usize {
        self.requests()
            .iter()
            .filter(|r| r.method == method && r.path.starts_with(prefix))
            .count()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.accept.take() {
            let _ = h.join();
        }
    }
}

fn serve(mut sock: TcpStream, seen: &Mutex<Vec<Req>>, handler: &dyn Fn(&Req) -> Resp) {
    let _ = sock.set_nonblocking(false);
    let _ = sock.set_read_timeout(Some(Duration::from_secs(10)));
    let Some(req) = read_request(&mut sock) else {
        return;
    };
    seen.lock().unwrap().push(req.clone());
    let resp = handler(&req);
    let extra: String = resp
        .headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}\r\n"))
        .collect();
    let out = format!(
        "HTTP/1.1 {} X\r\nContent-Type: {}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        resp.status,
        resp.content_type,
        resp.body.len(),
        resp.body
    );
    let _ = sock.write_all(out.as_bytes());
}

fn read_request(sock: &mut TcpStream) -> Option<Req> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = sock.read(&mut chunk).ok()?;
        buf.extend_from_slice(&chunk[..n]);
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..at]).to_string();
            let mut lines = head.lines();
            let first = lines.next()?;
            let mut parts = first.split_whitespace();
            let method = parts.next()?.to_string();
            let path = parts.next()?.to_string();
            let headers: Vec<(String, String)> = lines
                .filter_map(|l| l.split_once(':'))
                .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
                .collect();
            let len = headers
                .iter()
                .find(|(k, _)| k == "content-length")
                .and_then(|(_, v)| v.parse::<usize>().ok())
                .unwrap_or(0);
            if buf.len() >= at + 4 + len {
                return Some(Req {
                    method,
                    path,
                    headers,
                    body: String::from_utf8_lossy(&buf[at + 4..at + 4 + len]).into_owned(),
                });
            }
        }
        if n == 0 {
            return None;
        }
    }
}
