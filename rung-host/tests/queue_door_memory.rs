//! S1: what the host's `/v1/queue` door does with a request body.
//!
//! The body is never read for a request that is refused on its headers (no
//! token, another role's token, not JSON): the refusal is answered at once and
//! the connection closed, so a body of any size costs the host nothing. The
//! body is generated as it is sent (never held), against the real `rung-host`
//! binary; the host's peak resident memory is read from `/proc`. An item an
//! owner does send is stored whole, past the old 16 MiB request limit.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use rung_host::sim;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");
const OWNER_TOKEN: &str = "s1mem-owner-token-canary-5521";
const PEER_TOKEN: &str = "s1mem-peer-token-canary-7743";
/// The size of the generated body a refused request declares. Every refusal
/// must leave the host's memory where it was, whatever this is; it is 1 GiB
/// because the box this runs on is shared (a host that buffered it would hold
/// about twice that).
const HUGE: u64 = 1 << 30;

struct Server {
    child: Child,
    addr: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn(state: &Path) -> Server {
    let mut child = Command::new(BIN)
        .args([
            "sim",
            "--clock",
            "real",
            "--state",
            state.to_str().unwrap(),
            "--call-ms",
            "20,40",
            "--no-commit",
            "--acp-http",
            "127.0.0.1:0",
            "--acp-token-env",
            "owner=S1MEM_OWNER_TOKEN",
            "--acp-token-env",
            "peer=S1MEM_PEER_TOKEN",
        ])
        .env("S1MEM_OWNER_TOKEN", OWNER_TOKEN)
        .env("S1MEM_PEER_TOKEN", PEER_TOKEN)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rung-host");
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut addr = String::new();
    for _ in 0..50 {
        let mut line = String::new();
        if stderr.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Some(url) = line.split(" ACP HTTP at ").nth(1) {
            addr = url
                .trim()
                .strip_prefix("http://")
                .and_then(|s| s.strip_suffix("/acp"))
                .unwrap_or("")
                .to_string();
            break;
        }
    }
    assert!(!addr.is_empty(), "no listen line");
    std::thread::spawn(move || {
        let mut sink = String::new();
        while stderr.read_line(&mut sink).unwrap_or(0) > 0 {
            sink.clear();
        }
    });
    Server { child, addr }
}

/// The host's peak resident set, in bytes (`VmHWM`).
fn peak_rss(pid: u32) -> u64 {
    let s = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    let kb: u64 = s
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
        .unwrap();
    kb * 1024
}

/// Send a request declaring `len` body bytes, then generate zeros until the
/// host closes or `len` is sent. Returns the status line's code (0 if none) and
/// how many body bytes the host took.
fn post_generated(addr: &str, path: &str, headers: &[(&str, &str)], len: u64) -> (u16, u64) {
    let mut sock = TcpStream::connect(addr).unwrap();
    sock.set_write_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let mut head = format!("POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {len}\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    sock.write_all(head.as_bytes()).unwrap();
    let chunk = vec![0u8; 64 * 1024];
    let mut sent = 0u64;
    while sent < len {
        let n = chunk.len().min((len - sent) as usize);
        match sock.write(&chunk[..n]) {
            Ok(0) | Err(_) => break,
            Ok(w) => sent += w as u64,
        }
    }
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    while let Ok(n) = sock.read(&mut tmp) {
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let status = String::from_utf8_lossy(&buf)
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (status, sent)
}

#[test]
fn a_request_refused_on_its_headers_is_refused_without_reading_its_body() {
    sim::test_timeout(180);
    let guard = sim::temp_dir_guard("s1mem-refused");
    let s = spawn(guard.path());
    let pid = s.child.id();
    let before = peak_rss(pid);
    let bearer_peer = format!("Bearer {PEER_TOKEN}");
    let bearer_owner = format!("Bearer {OWNER_TOKEN}");
    type Case<'a> = (&'a str, Vec<(&'a str, &'a str)>, u16);
    let cases: Vec<Case> = vec![
        ("no token", vec![("Content-Type", "application/json")], 401),
        (
            "an unknown token",
            vec![
                ("Content-Type", "application/json"),
                ("Authorization", "Bearer nobody"),
            ],
            401,
        ),
        (
            "the peer's token",
            vec![
                ("Content-Type", "application/json"),
                ("Authorization", &bearer_peer),
            ],
            403,
        ),
        (
            "the owner's token but not JSON",
            vec![
                ("Content-Type", "text/plain"),
                ("Authorization", &bearer_owner),
            ],
            415,
        ),
    ];
    for (what, headers, want) in cases {
        let (status, sent) = post_generated(&s.addr, "/v1/queue", &headers, HUGE);
        assert_eq!(status, want, "{what}");
        assert!(
            sent < 64 << 20,
            "{what}: the host took {sent} bytes of a refused body"
        );
    }
    let grown = peak_rss(pid).saturating_sub(before);
    assert!(
        grown < 64 << 20,
        "refused bodies grew the host's peak memory by {grown} bytes"
    );
    // and it is still serving
    let r = reqwest::blocking::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://{}/v1/queue", s.addr))
        .bearer_auth(OWNER_TOKEN)
        .send()
        .unwrap();
    assert_eq!(r.status(), 200);
}

#[test]
fn a_body_to_a_path_the_queue_door_does_not_serve_is_not_read() {
    sim::test_timeout(180);
    let guard = sim::temp_dir_guard("s1mem-other");
    let s = spawn(guard.path());
    let pid = s.child.id();
    let before = peak_rss(pid);
    let bearer_owner = format!("Bearer {OWNER_TOKEN}");
    for path in ["/v1/events", "/v1/nowhere"] {
        for (what, auth) in [
            ("no token", None),
            ("the owner's token", Some(&bearer_owner)),
        ] {
            let mut headers = vec![("Content-Type", "application/json")];
            if let Some(a) = auth {
                headers.push(("Authorization", a.as_str()));
            }
            let (status, sent) = post_generated(&s.addr, path, &headers, HUGE);
            assert!(
                matches!(status, 401 | 403 | 404 | 405),
                "{path} with {what}: answered {status}"
            );
            assert!(
                sent < 64 << 20,
                "{path} with {what}: the host took {sent} bytes of a body"
            );
        }
    }
    let grown = peak_rss(pid).saturating_sub(before);
    assert!(
        grown < 64 << 20,
        "unserved bodies grew the host's peak memory by {grown} bytes"
    );
}

#[test]
fn an_item_past_the_old_request_limit_is_recorded_whole() {
    sim::test_timeout(180);
    let guard = sim::temp_dir_guard("s1mem-whole");
    let s = spawn(guard.path());
    let pid = s.child.id();
    let before = peak_rss(pid);
    // 40 MiB of text, well past the listener's old 16 MiB request limit
    let text: String = (0..40u64 << 20)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    let http = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap();
    let r = http
        .post(format!("http://{}/v1/queue", s.addr))
        .header("content-type", "application/json")
        .bearer_auth(OWNER_TOKEN)
        .body(json!({"text": text, "key": "k-huge"}).to_string())
        .send()
        .unwrap();
    assert_eq!(r.status().as_u16(), 201);
    drop(r);
    // the record holds it whole: one line, all of the text
    let seg: Vec<_> = std::fs::read_dir(guard.path().join("record"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    let mut found = 0;
    for p in seg {
        for l in BufReader::new(std::fs::File::open(p).unwrap()).lines() {
            let l = l.unwrap();
            if !l.contains("\"k-huge\"") {
                continue;
            }
            let v: Value = serde_json::from_str(&l).unwrap();
            // later lines name the item too (admitted, disposed): only its
            // acceptance holds the text
            if v["kind"] == "stimulus.accepted" {
                assert_eq!(v["item"]["text"].as_str().unwrap().len(), 40 << 20);
                found += 1;
            }
        }
    }
    assert_eq!(found, 1);
    // What it cost. An item is one JSON line the record holds and every reader
    // reads whole, and the inbox holds it until a turn reads it, so storing it
    // takes a bounded number of copies of it (the body, the parsed item, the
    // inbox's, the line written, the answer): about nine measured. That bound
    // is what is guarded here; it is not flat, and cannot be while an item's
    // text lives inside its record line.
    let grown = peak_rss(pid).saturating_sub(before);
    assert!(
        grown < (40u64 << 20) * 12,
        "the host's peak memory grew by {grown} bytes for a 40 MiB item"
    );
}
