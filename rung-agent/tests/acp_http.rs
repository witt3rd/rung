//! Streamable HTTP ACP against `rung-agent --acp-http` (no LLM).
//! Wire matches `@agentclientprotocol/sdk` experimental AcpServer.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rung-agent"))
}

struct Server {
    child: Child,
    host: String,
    port: u16,
    tmp: std::path::PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.tmp);
    }
}

fn spawn_http() -> Server {
    let tmp = tempfile();
    let mut child = bin()
        .arg("--acp-http")
        .arg("127.0.0.1:0")
        .current_dir(&tmp)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    stderr.read_line(&mut line).expect("listen line");
    let url = line
        .split(" at ")
        .nth(1)
        .unwrap_or_else(|| panic!("listen line: {line}"))
        .trim();
    let rest = url
        .strip_prefix("http://")
        .and_then(|s| s.strip_suffix("/acp"))
        .unwrap_or_else(|| panic!("url: {url}"));
    let (host, port_s) = rest.rsplit_once(':').expect("host:port");
    Server {
        child,
        host: host.to_string(),
        port: port_s.parse().unwrap(),
        tmp,
    }
}

struct Http {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn request(server: &Server, method: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Http {
    let mut stream = TcpStream::connect((server.host.as_str(), server.port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut req = format!(
        "{method} /acp HTTP/1.1\r\nHost: {}:{}\r\n",
        server.host, server.port
    );
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
    stream.flush().unwrap();
    read_http(stream)
}

fn read_http(stream: TcpStream) -> Http {
    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let mut body = Vec::new();
    if let Some(len) = headers.get("content-length") {
        let n: usize = len.parse().unwrap_or(0);
        body.resize(n, 0);
        if n > 0 {
            reader.read_exact(&mut body).unwrap();
        }
    } else {
        reader.read_to_end(&mut body).ok();
    }
    Http {
        status,
        headers,
        body,
    }
}

fn open_sse(
    server: &Server,
    headers: &[(&str, &str)],
) -> (u16, HashMap<String, String>, BufReader<TcpStream>) {
    let mut stream = TcpStream::connect((server.host.as_str(), server.port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut req = format!(
        "GET /acp HTTP/1.0\r\nHost: {}:{}\r\nAccept: text/event-stream\r\n",
        server.host, server.port
    );
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).unwrap();
    stream.flush().unwrap();
    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let mut hdrs = HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            hdrs.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    (status, hdrs, reader)
}

fn read_sse_json(reader: &mut BufReader<TcpStream>) -> Value {
    let mut data = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("sse line");
        if line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            data.push_str(rest.trim_start());
            if data.ends_with('\n') {
                data.pop();
            }
            if data.ends_with('\r') {
                data.pop();
            }
        } else if (line == "\n" || line == "\r\n" || line.is_empty()) && !data.is_empty() {
            return serde_json::from_str(data.trim())
                .unwrap_or_else(|e| panic!("sse json {e}: {data}"));
        }
    }
}

fn post_json(server: &Server, extra: &[(&str, &str)], body: &Value) -> Http {
    let bytes = serde_json::to_vec(body).unwrap();
    let owned: Vec<(String, String)> = extra
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    let mut hdrs: Vec<(&str, &str)> = vec![("Content-Type", "application/json")];
    for (k, v) in &owned {
        hdrs.push((k.as_str(), v.as_str()));
    }
    request(server, "POST", &hdrs, Some(&bytes))
}

fn tempfile() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rung-agent-acp-http-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn initialize_session_new_prompt_delete() {
    let server = spawn_http();
    let cwd = server.tmp.to_string_lossy().into_owned();

    let init = post_json(
        &server,
        &[],
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}),
    );
    assert_eq!(init.status, 200, "{}", String::from_utf8_lossy(&init.body));
    let conn = init
        .headers
        .get("acp-connection-id")
        .cloned()
        .expect("Acp-Connection-Id");
    assert_eq!(conn.len(), 36);
    let init_body: Value = serde_json::from_slice(&init.body).unwrap();
    assert_eq!(init_body["result"]["protocolVersion"], 1);
    assert_eq!(
        init_body["result"]["agentCapabilities"]["loadSession"],
        true
    );

    let (st, _, mut sse) = open_sse(&server, &[("Acp-Connection-Id", conn.as_str())]);
    assert_eq!(st, 200);

    let created = post_json(
        &server,
        &[("Acp-Connection-Id", conn.as_str())],
        &json!({
            "jsonrpc":"2.0","id":2,"method":"session/new",
            "params":{"cwd": cwd, "mcpServers":[]}
        }),
    );
    assert_eq!(
        created.status,
        202,
        "{}",
        String::from_utf8_lossy(&created.body)
    );
    let ev = read_sse_json(&mut sse);
    assert_eq!(ev["id"], 2);
    let sid = ev["result"]["sessionId"].as_str().expect("sessionId");
    assert!(!sid.is_empty());

    let (st, _, mut sess_sse) = open_sse(
        &server,
        &[
            ("Acp-Connection-Id", conn.as_str()),
            ("Acp-Session-Id", sid),
        ],
    );
    assert_eq!(st, 200);

    let prompt = post_json(
        &server,
        &[
            ("Acp-Connection-Id", conn.as_str()),
            ("Acp-Session-Id", sid),
        ],
        &json!({
            "jsonrpc":"2.0","id":3,"method":"session/prompt",
            "params":{"sessionId": sid, "prompt":[]}
        }),
    );
    assert_eq!(prompt.status, 202);
    let done = read_sse_json(&mut sess_sse);
    assert_eq!(done["id"], 3);
    assert_eq!(done["result"]["stopReason"], "end_turn");

    let del = request(
        &server,
        "DELETE",
        &[("Acp-Connection-Id", conn.as_str())],
        None,
    );
    assert_eq!(del.status, 202);
}

#[test]
fn http_refusals_match_acp_server() {
    let server = spawn_http();
    let missing = request(&server, "GET", &[("Accept", "text/event-stream")], None);
    assert_eq!(missing.status, 400);

    let bad_accept = request(&server, "GET", &[("Acp-Connection-Id", "x")], None);
    assert_eq!(bad_accept.status, 406);

    let unknown = request(
        &server,
        "GET",
        &[
            ("Accept", "text/event-stream"),
            ("Acp-Connection-Id", "no-such"),
        ],
        None,
    );
    assert_eq!(unknown.status, 404);

    let batch = request(
        &server,
        "POST",
        &[("Content-Type", "application/json")],
        Some(b"[]"),
    );
    assert_eq!(batch.status, 501);

    let not_json = request(
        &server,
        "POST",
        &[("Content-Type", "text/plain")],
        Some(b"{}"),
    );
    assert_eq!(not_json.status, 415);

    let ws = request(&server, "GET", &[("Upgrade", "websocket")], None);
    assert_eq!(ws.status, 426);

    let del = request(&server, "DELETE", &[], None);
    assert_eq!(del.status, 400);
}

#[test]
fn token_rejects_without_bearer() {
    let tmp = tempfile();
    let mut child = bin()
        .args(["--acp-http", "127.0.0.1:0", "--acp-token", "secret"])
        .current_dir(&tmp)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    stderr.read_line(&mut line).unwrap();
    let url = line.split(" at ").nth(1).unwrap().trim();
    let rest = url
        .strip_prefix("http://")
        .and_then(|s| s.strip_suffix("/acp"))
        .unwrap();
    let (host, port_s) = rest.rsplit_once(':').unwrap();
    let server = Server {
        child,
        host: host.to_string(),
        port: port_s.parse().unwrap(),
        tmp,
    };
    let denied = post_json(
        &server,
        &[],
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}),
    );
    assert_eq!(denied.status, 401);
    let ok = post_json(
        &server,
        &[("Authorization", "Bearer secret")],
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}),
    );
    assert_eq!(ok.status, 200, "{}", String::from_utf8_lossy(&ok.body));
}
