//! Golden-transcript parity for the `rung-agent` binary.
//!
//! Each fixture under `tests/fixtures/parity/` is one whole scripted session,
//! recorded from the v0.1.13 binary (the last release before `rung-agent`
//! became a thin shell over `rung-agent-core`). A fixture holds the inputs
//! (argv, env, stdin, files, the scripted replies of a mock OpenAI-compatible
//! model, a mock judge and a mock MCP server, every ACP line a client sent)
//! and everything the binary did that a caller can observe: each line it
//! wrote on stdout (CLI output, or ACP messages interleaved with the client's
//! lines), stderr, the exit code, and every request it made to the mocks.
//!
//! The test replays a fixture's inputs against the binary built now and
//! requires the observed outputs to be equal byte for byte. What differs per
//! run is written as a placeholder on both sides before the comparison: the
//! temp dir (`<tmp>`), the mock URLs (`<llm>`, `<jev>`, `<mcp>`), session ids
//! (`<id1>`, …) and uuids (`<uuid1>`, …), numbered by first appearance.
//!
//! ACP fixtures are replayed from the client lines the fixture holds, so an
//! ACP fixture is also a captured exchange of the v0.1.13 binary run against
//! the binary built now.
//!
//! Re-record only from the reference binary:
//!
//! ```text
//! RUNG_PARITY_RECORD=1 cargo test -p rung-agent --test parity
//! ```

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const RECORDED_FROM: &str = "rung-agent 0.1.13 (c0e23a8)";

// ─── Fixture shape ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Fixture {
    name: String,
    recorded_from: String,
    /// The child's whole environment besides `PATH` (placeholders allowed).
    env: BTreeMap<String, String>,
    /// Files written under `<tmp>` before the first step.
    files: BTreeMap<String, String>,
    /// The mock model's replies, in order. `__status` + `__body` answer with
    /// that HTTP status instead of a completion.
    llm: Vec<Value>,
    /// The mock judge's responses, in order.
    jev: Vec<Value>,
    /// The mock MCP server's `tools/call` error message (`None`: no server).
    mcp: Option<String>,
    steps: Vec<Step>,
    /// Every request the binary made to the mock model: request line, body.
    llm_requests: Vec<String>,
    jev_requests: Vec<String>,
    mcp_requests: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Step {
    argv: Vec<String>,
    /// CLI: bytes on stdin. ACP: unused.
    stdin: Option<String>,
    /// ACP when true: `> ` lines are sent, `< ` lines are what came back.
    acp: bool,
    /// CLI: `< ` stdout lines. ACP: the exchange in order.
    exchange: Vec<String>,
    stderr: Vec<String>,
    exit: Option<i32>,
}

// ─── Normalization ───────────────────────────────────────────────────────────

/// Per-run values ↔ placeholders. Learned ids are numbered by first sight.
#[derive(Default)]
struct Norm {
    fixed: Vec<(String, String)>,
    ids: Vec<(String, String)>,
    uuids: Vec<(String, String)>,
}

fn is_hex(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A `session::new_id` (`{nanos:x}-{pid:x}`).
fn is_session_id(t: &str) -> bool {
    let mut p = t.split('-');
    match (p.next(), p.next(), p.next()) {
        (Some(a), Some(b), None) => is_hex(a) && a.len() >= 12 && is_hex(b) && b.len() <= 8,
        _ => false,
    }
}

fn is_uuid(t: &str) -> bool {
    let p: Vec<&str> = t.split('-').collect();
    p.len() == 5
        && p.iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(s, n)| s.len() == n && is_hex(s))
}

impl Norm {
    fn new(tmp: &Path, urls: &[(&str, &str)]) -> Self {
        let mut fixed = Vec::new();
        let canon = tmp.canonicalize().unwrap();
        for p in [canon.as_path(), tmp] {
            fixed.push((p.to_string_lossy().into_owned(), "<tmp>".to_string()));
        }
        for (url, name) in urls {
            fixed.push((url.to_string(), format!("<{name}>")));
        }
        fixed.sort_by_key(|(real, _)| std::cmp::Reverse(real.len()));
        fixed.dedup();
        Norm {
            fixed,
            ..Norm::default()
        }
    }

    /// Learn the ids in `text`, then write every known value as its placeholder.
    fn apply(&mut self, text: &str) -> String {
        let mut out = text.to_string();
        for (real, ph) in &self.fixed {
            out = out.replace(real.as_str(), ph);
        }
        let mut result = String::with_capacity(out.len());
        let mut token = String::new();
        let flush = |token: &mut String, result: &mut String, me: &mut Norm| {
            if token.is_empty() {
                return;
            }
            let t = std::mem::take(token);
            let ph = if is_uuid(&t) {
                Some(learn(&mut me.uuids, &t, "uuid"))
            } else if is_session_id(&t) {
                Some(learn(&mut me.ids, &t, "id"))
            } else {
                None
            };
            result.push_str(ph.as_deref().unwrap_or(&t));
        };
        for c in out.chars() {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                token.push(c);
            } else {
                flush(&mut token, &mut result, self);
                result.push(c);
            }
        }
        flush(&mut token, &mut result, self);
        timings(&result)
    }

    /// Placeholders back to this run's values (for lines sent to the child).
    fn undo(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (real, ph) in self.ids.iter().chain(&self.uuids) {
            out = out.replace(ph.as_str(), real);
        }
        // `<tmp>` maps to two spellings; the first (canonical) is the one sent.
        let mut seen = Vec::new();
        for (real, ph) in &self.fixed {
            if !seen.contains(ph) {
                seen.push(ph.clone());
                out = out.replace(ph.as_str(), real);
            }
        }
        out
    }
}

/// Measured times, which differ on every run: their numbers become `<t>`.
const TIMED: [&str; 4] = [
    "ttft_ms",
    "duration_ms",
    "latency_ms",
    "output_tokens_per_second",
];

/// `YYYY-MM-DDTHH:MM:SSZ`, the clock time a record was kept at.
fn is_stamp(b: &[u8]) -> bool {
    b.len() >= 20
        && b[..20].iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

fn timings(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if is_stamp(rest.as_bytes()) {
            out.push_str("<time>");
            rest = &rest[20..];
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    for key in TIMED {
        for quote in ["\"", "\\\""] {
            let needle = format!("{key}{quote}:");
            let mut from = 0;
            while let Some(at) = out[from..].find(&needle) {
                let start = from + at + needle.len();
                let len = out[start..]
                    .find(|c: char| !(c.is_ascii_digit() || ".-+eE".contains(c)))
                    .unwrap_or(out.len() - start);
                if len > 0 {
                    out.replace_range(start..start + len, "<t>");
                }
                from = start;
            }
        }
    }
    out
}

fn learn(table: &mut Vec<(String, String)>, real: &str, tag: &str) -> String {
    if let Some((_, ph)) = table.iter().find(|(r, _)| r == real) {
        return ph.clone();
    }
    let ph = format!("<{tag}{}>", table.len() + 1);
    table.push((real.to_string(), ph.clone()));
    ph
}

// ─── Mocks ───────────────────────────────────────────────────────────────────

/// Read one HTTP request: its request line and body.
fn read_request(sock: &mut TcpStream) -> Option<(String, String)> {
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
                let line = text.lines().next().unwrap_or_default().to_string();
                let body = String::from_utf8_lossy(&buf[at + 4..at + 4 + len]).to_string();
                return Some((line, body));
            }
        }
    }
}

fn respond(sock: &mut TcpStream, code: u16, ctype: &str, payload: &str) {
    let resp = format!(
        "HTTP/1.1 {code} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    let _ = sock.write_all(resp.as_bytes());
}

type Log = Arc<Mutex<Vec<String>>>;

/// OpenAI-compatible model: each request gets the next reply, as SSE when
/// the request streams.
fn mock_llm(replies: Vec<Value>, log: Log) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for reply in replies {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let Some((line, body)) = read_request(&mut sock) else {
                continue;
            };
            log.lock().unwrap().push(format!("{line}\n{body}"));
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            if let Some(status) = reply["__status"].as_u64() {
                respond(
                    &mut sock,
                    status as u16,
                    "application/json",
                    &reply["__body"].to_string(),
                );
            } else if parsed["stream"] == true {
                let mut delta = reply["choices"][0]["message"].clone();
                if let Some(calls) = delta.get_mut("tool_calls").and_then(|c| c.as_array_mut()) {
                    for (i, c) in calls.iter_mut().enumerate() {
                        c["index"] = json!(i);
                    }
                }
                let chunk = json!({"id": reply["id"], "model": reply["model"],
                    "choices": [{"delta": delta, "finish_reason": reply["choices"][0]["finish_reason"]}]});
                respond(
                    &mut sock,
                    200,
                    "text/event-stream",
                    &format!("data: {chunk}\n\ndata: [DONE]\n\n"),
                );
            } else {
                respond(&mut sock, 200, "application/json", &reply.to_string());
            }
        }
    });
    format!("http://127.0.0.1:{port}/v1")
}

/// The judge (`POST {base}/systemone`): each request gets the next response.
fn mock_jev(replies: Vec<Value>, log: Log) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for reply in replies {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let Some((line, body)) = read_request(&mut sock) else {
                continue;
            };
            log.lock().unwrap().push(format!("{line}\n{body}"));
            respond(&mut sock, 200, "application/json", &reply.to_string());
        }
    });
    format!("http://127.0.0.1:{port}/api/v1")
}

/// A streamable-HTTP MCP server with one tool, `lookup`, whose every call
/// fails with `error` (a JSON-RPC error).
fn mock_mcp(error: String, log: Log) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for sock in listener.incoming() {
            let Ok(mut sock) = sock else { return };
            let Some((line, body)) = read_request(&mut sock) else {
                continue;
            };
            log.lock().unwrap().push(format!("{line}\n{body}"));
            let req: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let id = req["id"].clone();
            let reply = match req["method"].as_str().unwrap_or_default() {
                "initialize" => json!({"jsonrpc": "2.0", "id": id, "result": {
                    "protocolVersion": req["params"]["protocolVersion"],
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "parity-kv", "version": "1"}}}),
                "tools/list" => json!({"jsonrpc": "2.0", "id": id, "result": {"tools": [{
                    "name": "lookup",
                    "description": "Look up a key.",
                    "inputSchema": {"type": "object", "properties": {"key": {"type": "string"}}}
                }]}}),
                "tools/call" => json!({"jsonrpc": "2.0", "id": id,
                    "error": {"code": -32000, "message": error}}),
                _ => {
                    respond(&mut sock, 202, "application/json", "");
                    continue;
                }
            };
            respond(&mut sock, 200, "application/json", &reply.to_string());
        }
    });
    format!("http://127.0.0.1:{port}/mcp")
}

// ─── Running a fixture ───────────────────────────────────────────────────────

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rung-parity-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn read_line(r: &mut impl BufRead) -> Option<String> {
    let mut line = String::new();
    match r.read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim_end_matches('\n').to_string()),
    }
}

/// Run `f`'s inputs against the binary and fill in what it observed.
fn replay(f: &Fixture) -> Fixture {
    let tmp = tempdir(&f.name);
    for (rel, content) in &f.files {
        let p = tmp.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }
    let (llm_log, jev_log, mcp_log) = (Log::default(), Log::default(), Log::default());
    let llm = mock_llm(f.llm.clone(), llm_log.clone());
    let jev = mock_jev(f.jev.clone(), jev_log.clone());
    let mcp = f
        .mcp
        .clone()
        .map(|e| mock_mcp(e, mcp_log.clone()))
        .unwrap_or_else(|| "http://127.0.0.1:9/none".into());
    let mut norm = Norm::new(&tmp, &[(&llm, "llm"), (&jev, "jev"), (&mcp, "mcp")]);

    let mut out = f.clone();
    for step in &mut out.steps {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rung-agent"));
        cmd.env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .current_dir(&tmp)
            .args(step.argv.iter().map(|a| norm.undo(a)))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());
        for (k, v) in &f.env {
            cmd.env(k, norm.undo(v));
        }
        let mut child = cmd.spawn().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let err_reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s);
            s
        });
        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut exchange = Vec::new();
        if step.acp {
            let sends: Vec<String> = step
                .exchange
                .iter()
                .filter_map(|l| l.strip_prefix("> ").map(str::to_string))
                .collect();
            for send in sends {
                let real = norm.undo(&send);
                exchange.push(format!("> {send}"));
                writeln!(stdin, "{real}").unwrap();
                stdin.flush().unwrap();
                let msg: Value = serde_json::from_str(&real).unwrap();
                if msg.get("id").is_none() {
                    continue;
                }
                while let Some(line) = read_line(&mut stdout) {
                    exchange.push(format!("< {}", norm.apply(&line)));
                    let v: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
                    if v["id"] == msg["id"] && v.get("method").is_none() {
                        break;
                    }
                }
            }
        } else if let Some(input) = &step.stdin {
            stdin.write_all(norm.undo(input).as_bytes()).unwrap();
        }
        drop(stdin);
        while let Some(line) = read_line(&mut stdout) {
            exchange.push(format!("< {}", norm.apply(&line)));
        }
        let status = child.wait().unwrap();
        let err = err_reader.join().unwrap();
        step.exchange = exchange;
        step.stderr = err.lines().map(|l| norm.apply(l)).collect();
        step.exit = status.code();
    }
    let drain = |log: &Log, norm: &mut Norm| -> Vec<String> {
        log.lock().unwrap().iter().map(|r| norm.apply(r)).collect()
    };
    out.llm_requests = drain(&llm_log, &mut norm);
    out.jev_requests = drain(&jev_log, &mut norm);
    out.mcp_requests = drain(&mcp_log, &mut norm);
    let _ = std::fs::remove_dir_all(&tmp);
    out
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parity")
        .join(format!("{name}.json"))
}

/// Record (`RUNG_PARITY_RECORD=1`) or replay and compare one fixture.
fn check(name: &str) {
    let path = fixture_path(name);
    if std::env::var("RUNG_PARITY_RECORD").is_ok_and(|v| v == "1") {
        let got = replay(&scenario(name));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = serde_json::to_string_pretty(&got).unwrap() + "\n";
        std::fs::write(&path, text).unwrap();
        return;
    }
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (record it first)", path.display()));
    let golden: Fixture = serde_json::from_str(&text).unwrap();
    let got = replay(&golden);
    let got_text = serde_json::to_string_pretty(&got).unwrap() + "\n";
    if got_text != text {
        let first = text
            .lines()
            .zip(got_text.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| format!("line {}:\n  golden: {a}\n  now:    {b}", i + 1))
            .unwrap_or_else(|| "one is a prefix of the other".into());
        let dump = std::env::temp_dir().join(format!("rung-parity-{name}.json"));
        let _ = std::fs::write(&dump, &got_text);
        panic!(
            "{name}: the binary's observable output differs from the {} golden transcript\n{first}\nfull output: {}",
            golden.recorded_from,
            dump.display()
        );
    }
}

// ─── Scenarios (inputs; used when recording) ─────────────────────────────────

fn text_reply(text: &str) -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": text}, "finish_reason": "stop"}]})
}

fn call_reply(id: &str, name: &str, args: Value) -> Value {
    json!({"id": "c", "model": "m", "choices": [{"message": {"content": null, "tool_calls": [
        {"id": id, "type": "function", "function": {"name": name, "arguments": args.to_string()}}
    ]}, "finish_reason": "tool_calls"}]})
}

fn base_env() -> BTreeMap<String, String> {
    [
        ("HOME", "<tmp>"),
        ("XDG_CONFIG_HOME", "<tmp>"),
        ("RUNG_CONFIG", "<tmp>/none.yaml"),
        ("RUNG_HOME", "<tmp>"),
        ("RUNG_BASE_URL", "<llm>"),
        ("RUNG_MODEL", "m"),
        ("RUNG_API_KEY", "k"),
        ("RUNG_PROTOCOL", "openai"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

fn cli(argv: &[&str]) -> Step {
    Step {
        argv: argv.iter().map(|s| s.to_string()).collect(),
        stdin: None,
        acp: false,
        exchange: Vec::new(),
        stderr: Vec::new(),
        exit: None,
    }
}

/// An ACP step: `--acp` plus `extra` flags, sending `msgs` in order.
fn acp(extra: &[&str], msgs: Vec<Value>) -> Step {
    let mut argv = vec!["--acp".to_string()];
    argv.extend(extra.iter().map(|s| s.to_string()));
    Step {
        argv,
        stdin: None,
        acp: true,
        exchange: msgs.iter().map(|m| format!("> {m}")).collect(),
        stderr: Vec::new(),
        exit: None,
    }
}

fn rpc(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

fn ask(id: u64, sid: &str, text: &str) -> Value {
    rpc(
        id,
        "session/prompt",
        json!({"sessionId": sid, "prompt": [{"type": "text", "text": text}]}),
    )
}

fn judge(outcome: &str, claims: f64) -> Value {
    json!({"model": "judge", "answers": {
        "claims_unperformed_action": {"type": "noul", "noul": claims},
        "hides_failure": {"type": "noul", "noul": 0.05},
        "outcome": {"type": "choice", "choice": outcome,
            "probabilities": {"answered": 0, "asked_user": 0, "narrated": if outcome == "narrated" { 1 } else { 0 },
                "blocked": 0, "done": if outcome == "done" { 1 } else { 0 }},
            "confidence": 0.95},
        "relies_on_prior_turn": {"type": "noul", "noul": 0.05},
        "request_needs_action": {"type": "noul", "noul": 0.95}
    }})
}

fn fixture(name: &str) -> Fixture {
    Fixture {
        name: name.into(),
        recorded_from: RECORDED_FROM.into(),
        env: base_env(),
        files: BTreeMap::new(),
        llm: Vec::new(),
        jev: Vec::new(),
        mcp: None,
        steps: Vec::new(),
        llm_requests: Vec::new(),
        jev_requests: Vec::new(),
        mcp_requests: Vec::new(),
    }
}

/// The ACP preamble: initialize, then `session/new` in `<tmp>` (id 2).
fn acp_open(mcp: Value, meta: Option<Value>) -> Vec<Value> {
    let mut params = json!({"cwd": "<tmp>", "mcpServers": mcp});
    if let Some(m) = meta {
        params["_meta"] = m;
    }
    vec![
        rpc(1, "initialize", json!({"protocolVersion": 1})),
        rpc(2, "session/new", params),
    ]
}

fn scenario(name: &str) -> Fixture {
    let mut f = fixture(name);
    match name {
        // Plain answer, text out.
        "cli_text" => {
            f.llm = vec![text_reply("hello there")];
            f.steps = vec![cli(&["--tools", "none", "say hello"])];
        }
        // A tool round, `--json` outcome, then a resumed turn replaying
        // history, then a poll of the recorded session.
        "cli_json_resume_poll" => {
            f.files.insert("notes/a.txt".into(), "alpha\nbeta\n".into());
            f.llm = vec![
                call_reply("call_1", "read_file", json!({"path": "notes/a.txt"})),
                text_reply("a.txt holds alpha and beta"),
                text_reply("still alpha and beta"),
            ];
            f.steps = vec![
                cli(&[
                    "--tools",
                    "read",
                    "--json",
                    "--task-id",
                    "t-parity",
                    "what is in notes/a.txt?",
                ]),
                cli(&["--tools", "read", "--task-id", "t-parity", "and now?"]),
                cli(&["--task-id", "t-parity"]),
            ];
        }
        // NDJSON stream with a tool call; system and user prompt files.
        "cli_stream" => {
            f.files.insert("b.txt".into(), "gamma\n".into());
            f.files.insert("sys.md".into(), "You are terse.".into());
            f.llm = vec![
                call_reply("call_1", "read_file", json!({"path": "b.txt"})),
                text_reply("gamma"),
            ];
            f.steps = vec![cli(&[
                "--tools",
                "read",
                "--stream",
                "--system-prompt",
                "@sys.md",
                "--user-prompt",
                "## brief\nread b.txt",
                "--task-id",
                "t-stream",
                "go",
            ])];
        }
        // Prompt on stdin.
        "cli_stdin" => {
            f.llm = vec![text_reply("from stdin")];
            let mut s = cli(&["--tools", "none", "--json"]);
            s.stdin = Some("  the prompt  \n".into());
            f.steps = vec![s];
        }
        // Exit codes without a model: help 0, a bad flag 2, no prompt 2, a
        // poll of an unknown session 1, a bad session id 1.
        "cli_exit_codes" => {
            let mut empty = cli(&["--tools", "none"]);
            empty.stdin = Some(String::new());
            f.steps = vec![
                cli(&["--help"]),
                cli(&["--no-such-flag"]),
                empty,
                cli(&["--task-id", "missing-session"]),
                cli(&["--task-id", "../escape", "hi"]),
                cli(&["--memory-check"]),
            ];
        }
        // A provider auth failure, plain and as `--stream`.
        "cli_provider_failure" => {
            let denied = json!({"__status": 401, "__body": {"error": {"message": "bad key"}}});
            f.llm = vec![denied.clone(), denied];
            f.steps = vec![
                cli(&["--tools", "none", "--task-id", "t-fail", "hi"]),
                cli(&["--tools", "none", "--stream", "--task-id", "t-fail2", "hi"]),
            ];
        }
        // Turn check on: narrated, nudged once, then acted and completed.
        "cli_turn_check" => {
            f.env.insert("RUNG_TURN_CHECK".into(), "jev".into());
            f.env
                .insert("RUNG_TURN_CHECK_BASE_URL".into(), "<jev>".into());
            f.env
                .insert("OPENROUTER_API_KEY".into(), "test-judge-key".into());
            f.llm = vec![
                text_reply("I wrote hello to notes.txt."),
                call_reply(
                    "call_1",
                    "write_file",
                    json!({"path": "notes.txt", "content": "hello\n"}),
                ),
                text_reply("I wrote hello to notes.txt."),
            ];
            f.jev = vec![judge("narrated", 0.98), judge("done", 0.05)];
            f.steps = vec![cli(&[
                "--tools",
                "write",
                "--json",
                "--task-id",
                "t-check",
                "Write hello to notes.txt.",
            ])];
        }
        // Baseline memory: retain in one session, recall in the next.
        "cli_memory_baseline" => {
            f.llm = vec![
                text_reply("The harbor code is 4417."),
                text_reply("You told me 4417."),
            ];
            f.steps = vec![
                cli(&[
                    "--tools",
                    "none",
                    "--memory",
                    "baseline",
                    "--json",
                    "--task-id",
                    "t-mem-a",
                    "Remember: the harbor code is 4417.",
                ]),
                cli(&[
                    "--tools",
                    "none",
                    "--memory",
                    "baseline",
                    "--json",
                    "--task-id",
                    "t-mem-b",
                    "What is the harbor code?",
                ]),
            ];
        }
        // An MCP tool whose error carries a secret named by RUNG_REDACT_ENVS.
        "cli_mcp_redaction" => {
            f.env
                .insert("RUNG_REDACT_ENVS".into(), "PARITY_TOKEN".into());
            f.env
                .insert("PARITY_TOKEN".into(), "sekret-token-8812".into());
            f.mcp = Some("backend refused token sekret-token-8812".into());
            f.llm = vec![
                call_reply("call_1", "lookup", json!({"key": "k1"})),
                text_reply("the lookup failed"),
            ];
            f.steps = vec![cli(&[
                "--tools",
                "none",
                "--mcp-http",
                "kv=<mcp>",
                "--json",
                "--task-id",
                "t-mcp",
                "look up k1",
            ])];
        }
        // ACP: the session surface, two turns (a tool round, then a turn
        // replaying it), and the handlers around them.
        "acp_session" => {
            f.files.insert("c.txt".into(), "delta\n".into());
            f.llm = vec![
                call_reply("call_1", "read_file", json!({"path": "c.txt"})),
                text_reply("c.txt holds delta"),
                text_reply("yes, delta"),
            ];
            let mut msgs = acp_open(json!([]), Some(json!({"systemPrompt": "Session rules."})));
            msgs.extend([
                rpc(3, "session/list", json!({"cwd": "<tmp>"})),
                ask(4, "<id1>", "what is in c.txt?"),
                ask(5, "<id1>", "are you sure?"),
                rpc(
                    6,
                    "session/set_mode",
                    json!({"sessionId": "<id1>", "modeId": "explore"}),
                ),
                rpc(
                    7,
                    "session/load",
                    json!({"sessionId": "<id1>", "cwd": "<tmp>", "mcpServers": []}),
                ),
                rpc(
                    8,
                    "session/fork",
                    json!({"sessionId": "<id1>", "cwd": "<tmp>", "mcpServers": []}),
                ),
                rpc(
                    9,
                    "session/resume",
                    json!({"sessionId": "<id1>", "cwd": "<tmp>", "mcpServers": []}),
                ),
                rpc(10, "session/prompt", json!({"sessionId": "<id1>", "prompt": []})),
                json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": "<id1>"}}),
                rpc(11, "session/close", json!({"sessionId": "<id1>"})),
                rpc(12, "session/delete", json!({"sessionId": "<id2>"})),
            ]);
            f.steps = vec![acp(&["--tools", "read"], msgs)];
        }
        // ACP typed terminal states: refused, auth failure, cap forced.
        "acp_terminals" => {
            let refusal = json!({"id": "c", "model": "m", "choices": [{"message": {
                "content": null, "refusal": "I can't help with that."}, "finish_reason": "stop"}]});
            let denied = json!({"__status": 401, "__body": {"error": {"message": "bad key"}}});
            f.files.insert("d.txt".into(), "epsilon\n".into());
            f.llm = vec![
                refusal,
                denied,
                call_reply("call_1", "read_file", json!({"path": "d.txt"})),
                text_reply("forced answer"),
            ];
            let mut msgs = acp_open(json!([]), None);
            msgs.extend([
                ask(3, "<id1>", "do something bad"),
                ask(4, "<id1>", "try again"),
                ask(5, "<id1>", "read d.txt"),
            ]);
            f.steps = vec![acp(&["--tools", "read", "--max-iterations", "1"], msgs)];
        }
        // ACP memory extension: baseline retains and recalls across sessions.
        "acp_memory" => {
            f.llm = vec![
                text_reply("Noted: the gate opens at nine."),
                text_reply("At nine."),
            ];
            let mut msgs = acp_open(json!([]), None);
            msgs.extend([
                ask(3, "<id1>", "Remember: the gate opens at nine."),
                rpc(4, "session/new", json!({"cwd": "<tmp>", "mcpServers": []})),
                ask(5, "<id2>", "When does the gate open?"),
            ]);
            f.steps = vec![acp(&["--tools", "none", "--memory", "baseline"], msgs)];
        }
        // ACP MCP over HTTP from session/new, with a redacted tool error, and
        // the turn check reading the turn as unverified.
        "acp_mcp_turn_check" => {
            f.env
                .insert("RUNG_REDACT_ENVS".into(), "PARITY_TOKEN".into());
            f.env
                .insert("PARITY_TOKEN".into(), "sekret-token-8812".into());
            f.env.insert("RUNG_TURN_CHECK".into(), "jev".into());
            f.env
                .insert("RUNG_TURN_CHECK_BASE_URL".into(), "<jev>".into());
            f.env
                .insert("OPENROUTER_API_KEY".into(), "test-judge-key".into());
            f.mcp = Some("backend refused token sekret-token-8812".into());
            f.llm = vec![
                call_reply("call_1", "lookup", json!({"key": "k2"})),
                text_reply("I stored k2."),
                text_reply("I stored k2."),
            ];
            f.jev = vec![judge("narrated", 0.98), judge("narrated", 0.98)];
            let servers = json!([{"type": "http", "name": "kv", "url": "<mcp>", "headers": []}]);
            let mut msgs = acp_open(servers, None);
            msgs.push(ask(3, "<id1>", "store k2"));
            f.steps = vec![acp(&["--tools", "none"], msgs)];
        }
        other => panic!("no scenario {other}"),
    }
    f
}

#[test]
fn cli_text() {
    check("cli_text");
}

#[test]
fn cli_json_resume_poll() {
    check("cli_json_resume_poll");
}

#[test]
fn cli_stream() {
    check("cli_stream");
}

#[test]
fn cli_stdin() {
    check("cli_stdin");
}

#[test]
fn cli_exit_codes() {
    check("cli_exit_codes");
}

#[test]
fn cli_provider_failure() {
    check("cli_provider_failure");
}

#[test]
fn cli_turn_check() {
    check("cli_turn_check");
}

#[test]
fn cli_memory_baseline() {
    check("cli_memory_baseline");
}

#[test]
fn cli_mcp_redaction() {
    check("cli_mcp_redaction");
}

#[test]
fn acp_session() {
    check("acp_session");
}

#[test]
fn acp_terminals() {
    check("acp_terminals");
}

#[test]
fn acp_memory() {
    check("acp_memory");
}

#[test]
fn acp_mcp_turn_check() {
    check("acp_mcp_turn_check");
}
