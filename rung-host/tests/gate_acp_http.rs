//! G-q: ACP over Streamable HTTP with per-role bearer tokens, against the
//! `rung-host` binary (`sim --acp-http`, real clock, mock engine). The
//! client speaks the wire `@agentclientprotocol/sdk` speaks: `POST /acp`
//! with `Acp-Connection-Id` / `Acp-Session-Id`, SSE `GET` streams.

mod common;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use common::*;
use rung_host::gates::{self, HttpAcpRun};
use rung_host::record::{Line, Record};
use rung_host::sim;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");
const OWNER_TOKEN: &str = "g-q-owner-token-4f1c";
const PEER_TOKEN: &str = "g-q-peer-token-9a2e";

fn command(state: &Path) -> Command {
    let mut c = Command::new(BIN);
    c.args([
        "sim",
        "--clock",
        "real",
        "--state",
        state.to_str().unwrap(),
        "--call-ms",
        "20,40",
        "--no-commit",
    ]);
    c
}

struct Server {
    child: Child,
    addr: String,
}

fn spawn(state: &Path) -> Server {
    let mut child = command(state)
        .args([
            "--acp-http",
            "127.0.0.1:0",
            "--acp-token-env",
            "owner=G_Q_OWNER_TOKEN",
            "--acp-token-env",
            "peer=G_Q_PEER_TOKEN",
        ])
        .env("G_Q_OWNER_TOKEN", OWNER_TOKEN)
        .env("G_Q_PEER_TOKEN", PEER_TOKEN)
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
    // Keep draining stderr so the host never blocks on it.
    std::thread::spawn(move || {
        let mut sink = String::new();
        while stderr.read_line(&mut sink).unwrap_or(0) > 0 {
            sink.clear();
        }
    });
    Server { child, addr }
}

struct Http {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn request(addr: &str, method: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Http {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut req = format!("{method} /acp HTTP/1.1\r\nHost: {addr}\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    if let Some(b) = body {
        req.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    req.push_str("Connection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).unwrap();
    if let Some(b) = body {
        stream.write_all(b).unwrap();
    }
    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let mut body = Vec::new();
    match headers.get("content-length").and_then(|n| n.parse::<usize>().ok()) {
        Some(n) => {
            body.resize(n, 0);
            reader.read_exact(&mut body).unwrap();
        }
        None => {
            reader.read_to_end(&mut body).ok();
        }
    }
    Http {
        status,
        headers,
        body,
    }
}

fn post(addr: &str, token: Option<&str>, conn: Option<&str>, sess: Option<&str>, m: &Value) -> Http {
    let auth = token.map(|t| format!("Bearer {t}"));
    let mut h: Vec<(&str, &str)> = vec![("Content-Type", "application/json")];
    if let Some(a) = &auth {
        h.push(("Authorization", a));
    }
    if let Some(c) = conn {
        h.push(("Acp-Connection-Id", c));
    }
    if let Some(s) = sess {
        h.push(("Acp-Session-Id", s));
    }
    request(addr, "POST", &h, Some(&serde_json::to_vec(m).unwrap()))
}

/// Open an SSE stream; its JSON events arrive on the receiver. Returns the
/// status too.
fn sse(addr: &str, token: &str, conn: &str, sess: Option<&str>) -> (u16, Receiver<Value>) {
    let mut stream = TcpStream::connect(addr).unwrap();
    let mut req = format!(
        "GET /acp HTTP/1.0\r\nHost: {addr}\r\nAccept: text/event-stream\r\nAuthorization: Bearer {token}\r\nAcp-Connection-Id: {conn}\r\n"
    );
    if let Some(s) = sess {
        req.push_str(&format!("Acp-Session-Id: {s}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).unwrap();
    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
    }
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut data = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            if let Some(rest) = line.strip_prefix("data:") {
                data.push_str(rest.trim());
            } else if line.trim().is_empty() && !data.is_empty() {
                if let Ok(v) = serde_json::from_str::<Value>(&data) {
                    let _ = tx.send(v);
                }
                data.clear();
            }
        }
    });
    (status, rx)
}

fn reply(rx: &Receiver<Value>, id: u64, secs: u64) -> Value {
    let limit = Duration::from_secs_f64(secs as f64 * load_factor());
    let t = Instant::now();
    while t.elapsed() < limit {
        if let Ok(v) = rx.recv_timeout(Duration::from_millis(50))
            && v["id"] == id
            && v.get("method").is_none()
        {
            return v;
        }
    }
    panic!("no reply to {id}");
}

fn lines(state: &Path) -> Vec<Line> {
    Record::read_dir(state.join("record")).unwrap_or_default()
}

fn init(addr: &str, token: &str) -> (u16, String) {
    let r = post(
        addr,
        Some(token),
        None,
        None,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1}}),
    );
    let conn = r.headers.get("acp-connection-id").cloned().unwrap_or_default();
    (r.status, conn)
}

#[test]
fn acp_over_http_caps_each_connection_by_its_token() {
    sim::test_timeout(600);
    let mut run = HttpAcpRun {
        tokens: vec![OWNER_TOKEN.into(), PEER_TOKEN.into()],
        ..HttpAcpRun::default()
    };

    // Starts without usable tokens are refused.
    let refused = sim::temp_dir_guard("gate-q-refused");
    let code = |c: &mut Command| c.stdout(Stdio::null()).stderr(Stdio::null()).status().unwrap();
    let no_tokens = code(command(refused.path()).args(["--acp-http", "127.0.0.1:0"]));
    run.refused_starts
        .insert("no_tokens".into(), no_tokens.code().unwrap_or(-1));
    let unset = code(
        command(refused.path())
            .args(["--acp-http", "127.0.0.1:0", "--acp-token-env", "owner=G_Q_UNSET"])
            .env_remove("G_Q_UNSET"),
    );
    run.refused_starts
        .insert("unset_token_env".into(), unset.code().unwrap_or(-1));

    let guard = sim::temp_dir_guard("gate-q");
    let state = guard.path().to_path_buf();
    let cwd = state.to_string_lossy().to_string();
    let mut server = spawn(&state);
    let addr = server.addr.clone();
    run.bound = addr.clone();
    let hello = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1}});
    run.status.insert(
        "no_bearer".into(),
        post(&addr, None, None, None, &hello).status,
    );
    run.status.insert(
        "bad_bearer".into(),
        post(&addr, Some("not-a-token"), None, None, &hello).status,
    );
    let (s, owner_conn) = init(&addr, OWNER_TOKEN);
    run.status.insert("owner_init".into(), s);
    let (s, peer_conn) = init(&addr, PEER_TOKEN);
    run.status.insert("peer_init".into(), s);
    let (_, owner_rx) = sse(&addr, OWNER_TOKEN, &owner_conn, None);
    let (_, peer_rx) = sse(&addr, PEER_TOKEN, &peer_conn, None);

    let new = |token: &str, conn: &str, id: u64, role: &str| {
        let r = post(
            &addr,
            Some(token),
            Some(conn),
            None,
            &json!({"jsonrpc": "2.0", "id": id, "method": "session/new",
                    "params": {"cwd": cwd, "mcpServers": [], "_meta": {"rung": {"role": role, "channel": "alice"}}}}),
        );
        assert_eq!(r.status, 202, "{}", String::from_utf8_lossy(&r.body));
    };
    new(PEER_TOKEN, &peer_conn, 2, "owner");
    run.replies
        .insert("peer_opens_owner".into(), reply(&peer_rx, 2, 20));
    new(PEER_TOKEN, &peer_conn, 3, "peer");
    let peer_new = reply(&peer_rx, 3, 20);
    run.replies.insert("peer_opens_peer".into(), peer_new.clone());
    new(OWNER_TOKEN, &owner_conn, 4, "owner");
    let owner_new = reply(&owner_rx, 4, 20);
    run.replies
        .insert("owner_opens_owner".into(), owner_new.clone());
    let peer_sid = peer_new["result"]["sessionId"].as_str().unwrap_or("").to_string();
    let owner_sid = owner_new["result"]["sessionId"].as_str().unwrap_or("").to_string();

    // Another valid token on the owner's connection.
    let switched = post(
        &addr,
        Some(PEER_TOKEN),
        Some(&owner_conn),
        None,
        &json!({"jsonrpc": "2.0", "id": 5, "method": "session/list", "params": {}}),
    );
    run.status.insert("switched_post".into(), switched.status);
    let (s, _) = sse(&addr, PEER_TOKEN, &owner_conn, Some(&owner_sid));
    run.status.insert("switched_get".into(), s);
    let del = request(
        &addr,
        "DELETE",
        &[
            ("Authorization", &format!("Bearer {PEER_TOKEN}")),
            ("Acp-Connection-Id", &owner_conn),
        ],
        None,
    );
    run.status.insert("switched_delete".into(), del.status);

    // A peer prompt, answered on the peer's session stream.
    let (_, peer_sess) = sse(&addr, PEER_TOKEN, &peer_conn, Some(&peer_sid));
    let (_, owner_sess) = sse(&addr, OWNER_TOKEN, &owner_conn, Some(&owner_sid));
    let r = post(
        &addr,
        Some(PEER_TOKEN),
        Some(&peer_conn),
        Some(&peer_sid),
        &json!({"jsonrpc": "2.0", "id": 6, "method": "session/prompt",
                "params": {"sessionId": peer_sid, "prompt": [{"type": "text", "text": "hi over http"}]}}),
    );
    assert_eq!(r.status, 202);
    run.replies
        .insert("peer_prompt".into(), reply(&peer_sess, 6, 120));

    // The peer's stop is refused; the owner's halts the host.
    post(
        &addr,
        Some(PEER_TOKEN),
        Some(&peer_conn),
        Some(&peer_sid),
        &json!({"jsonrpc": "2.0", "id": 7, "method": "_rung/stop", "params": {"sessionId": peer_sid}}),
    );
    run.replies
        .insert("peer_stop".into(), reply(&peer_sess, 7, 20));
    let t = Instant::now();
    post(
        &addr,
        Some(OWNER_TOKEN),
        Some(&owner_conn),
        Some(&owner_sid),
        &json!({"jsonrpc": "2.0", "id": 8, "method": "_rung/stop", "params": {"sessionId": owner_sid}}),
    );
    run.replies
        .insert("owner_stop".into(), reply(&owner_sess, 8, 20));
    let limit = Duration::from_secs_f64(30.0 * load_factor());
    let code = loop {
        if let Some(s) = server.child.try_wait().unwrap() {
            break s.code().unwrap_or(-1);
        }
        assert!(t.elapsed() < limit, "the host did not exit after the owner's stop");
        std::thread::sleep(Duration::from_millis(10));
    };
    run.exit = Some((code, t.elapsed().as_millis() as u64));

    let ls = lines(&state);
    let g = gates::g_q(&ls, &run);
    assert_gate_in(&state, &ls, &g);
    assert_gate_in(&state, &ls, &gates::g_k(&ls));
}
