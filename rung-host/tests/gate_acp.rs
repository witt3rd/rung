//! G-p: ACP outward, against the `rung-host` binary on stdio, on the real
//! clock with the mock engine. The test is one ACP client speaking JSON-RPC
//! lines: three channels (owner, peer, observer), prompts answered when
//! their turn ends, a cancel, an agent-initiated message, the `_rung/*`
//! extensions by role, list and load, and the owner's stop. G-k over the
//! record.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use common::*;
use rung_host::gates::{self, AcpRun};
use rung_host::record::{Line, Record};
use rung_host::sim;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");

struct Client {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
    run: AcpRun,
    next: u64,
}

impl Client {
    fn start(state: &Path) -> Self {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(state.join("host.stderr"))
            .ok();
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
                "--acp",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log.map_or_else(Stdio::null, Stdio::from))
            .spawn()
            .expect("spawn rung-host");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { return };
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    let _ = tx.send(v);
                }
            }
        });
        Self {
            child,
            stdin,
            rx,
            run: AcpRun::default(),
            next: 1,
        }
    }

    fn send(&mut self, m: Value) {
        let mut text = m.to_string();
        text.push('\n');
        self.stdin.write_all(text.as_bytes()).unwrap();
        self.stdin.flush().unwrap();
        self.run.wire.push((true, m));
    }

    fn request(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        id
    }

    fn drain(&mut self, wait: Duration) {
        if let Ok(v) = self.rx.recv_timeout(wait) {
            self.run.wire.push((false, v));
        }
        while let Ok(v) = self.rx.try_recv() {
            self.run.wire.push((false, v));
        }
    }

    /// Read until `f` holds of a message received (from `from` on).
    fn until(&mut self, secs: u64, what: &str, f: impl Fn(&Value) -> bool) -> Value {
        let limit = Duration::from_secs_f64(secs as f64 * load_factor());
        let t = Instant::now();
        let mut seen = 0;
        loop {
            let got: Vec<Value> = self.run.wire[seen..]
                .iter()
                .filter(|(out, _)| !*out)
                .map(|(_, m)| m.clone())
                .collect();
            seen = self.run.wire.len();
            if let Some(m) = got.into_iter().find(|m| f(m)) {
                return m;
            }
            assert!(t.elapsed() < limit, "timed out waiting for {what}");
            self.drain(Duration::from_millis(20));
        }
    }

    fn response(&mut self, id: u64, secs: u64) -> Value {
        if let Some((_, m)) = self
            .run
            .wire
            .iter()
            .find(|(out, m)| !*out && m["id"] == id && m.get("method").is_none())
        {
            return m.clone();
        }
        self.until(secs, &format!("the response to {id}"), |m| {
            m["id"] == id && m.get("method").is_none()
        })
    }
}

fn lines(state: &Path) -> Vec<Line> {
    Record::read_dir(state.join("record")).unwrap_or_default()
}

fn wait_record(state: &Path, secs: u64, what: &str, f: impl Fn(&[Line]) -> bool) {
    let limit = Duration::from_secs_f64(secs as f64 * load_factor());
    let t = Instant::now();
    while t.elapsed() < limit {
        if f(&lines(state)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for {what}");
}

#[test]
fn acp_outward_serves_channels_to_one_agent() {
    sim::test_timeout(600);
    let guard = sim::temp_dir_guard("gate-p");
    let state = guard.path().to_path_buf();
    let mut c = Client::start(&state);
    let cwd = state.to_string_lossy().to_string();

    let init = c.request(
        "initialize",
        json!({"protocolVersion": 1, "clientCapabilities": {}, "_meta": {"rung": {"outbox": true}}}),
    );
    c.response(init, 20);
    let new = |c: &mut Client, role: &str, channel: &str| -> String {
        let id = c.request(
            "session/new",
            json!({"cwd": cwd, "mcpServers": [], "_meta": {"rung": {"role": role, "channel": channel}}}),
        );
        c.response(id, 20)["result"]["sessionId"]
            .as_str()
            .expect("a session id")
            .to_string()
    };
    let owner = new(&mut c, "owner", "");
    let peer = new(&mut c, "peer", "alice");
    let observer = new(&mut c, "observer", "watch");

    // A stimulus with no reply asked: durable before its ack.
    let s = c.request(
        "_rung/stimulus",
        json!({"sessionId": owner, "text": "say hello to the owner"}),
    );
    let ack = c.response(s, 20);
    let item = ack["result"]["id"].as_str().unwrap_or("").to_string();
    let on_disk = lines(&state).iter().any(|l| {
        l.kind == "stimulus.accepted" && l.get("item")["id"].as_str() == Some(item.as_str())
    });
    c.run.durable_acks.push((item, on_disk));
    // The turn that answers it has no open prompt from the owner: its send
    // is agent-initiated.
    c.until(60, "an _rung/outbox notification", |m| {
        m["method"] == "_rung/outbox"
    });

    // Two channels' prompts, back to back; an observer's, refused.
    let p1 = c.request(
        "session/prompt",
        json!({"sessionId": peer, "prompt": [{"type": "text", "text": "hi from alice"}]}),
    );
    let p2 = c.request(
        "session/prompt",
        json!({"sessionId": owner, "prompt": [{"type": "text", "text": "what are you doing"}]}),
    );
    let p3 = c.request(
        "session/prompt",
        json!({"sessionId": observer, "prompt": [{"type": "text", "text": "may I speak"}]}),
    );
    c.response(p3, 20);
    c.response(p1, 120);
    c.response(p2, 120);

    // Owner-only extensions, from a peer: refused.
    for (m, p) in [
        ("_rung/stop", json!({"sessionId": peer})),
        (
            "_rung/calendar",
            json!({"sessionId": peer, "id": "peer-item", "in_s": 1, "text": "x"}),
        ),
        ("_rung/release", json!({"sessionId": peer, "reason": "x"})),
    ] {
        let id = c.request(m, p);
        c.response(id, 20);
    }
    // The owner's calendar entry; the observer's status.
    let cal = c.request(
        "_rung/calendar",
        json!({"sessionId": owner, "id": "oven", "in_s": 1, "text": "check the oven", "firm": true}),
    );
    c.response(cal, 20);
    let st = c.request("_rung/status", json!({"sessionId": observer}));
    c.response(st, 20);

    // A prompt cancelled at once.
    let p4 = c.request(
        "session/prompt",
        json!({"sessionId": peer, "prompt": [{"type": "text", "text": "never mind"}]}),
    );
    c.send(json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": peer}}));
    c.response(p4, 60);

    let l = c.request("session/list", json!({}));
    c.response(l, 20);
    let ld = c.request(
        "session/load",
        json!({"sessionId": peer, "cwd": cwd, "mcpServers": []}),
    );
    c.response(ld, 20);
    wait_record(&state, 30, "the owner's calendar entry to fire", |ls| {
        ls.iter()
            .any(|l| l.kind == "calendar.fired" && l.str("id") == "oven")
    });

    // An open prompt, then the owner's stop.
    let p5 = c.request(
        "session/prompt",
        json!({"sessionId": peer, "prompt": [{"type": "text", "text": "one more"}]}),
    );
    let stop = c.request("_rung/stop", json!({"sessionId": owner}));
    let t = Instant::now();
    c.response(stop, 20);
    c.response(p5, 20);
    let limit = Duration::from_secs_f64(30.0 * load_factor());
    let code = loop {
        if let Some(status) = c.child.try_wait().unwrap() {
            break status.code().unwrap_or(-1);
        }
        assert!(
            t.elapsed() < limit,
            "the host did not exit after the owner's stop"
        );
        c.drain(Duration::from_millis(10));
    };
    c.run.exit = Some((code, t.elapsed().as_millis() as u64));
    c.drain(Duration::from_millis(50));

    let ls = lines(&state);
    let g = gates::g_p(&ls, &c.run);
    assert_gate_in(&state, &ls, &g);
    assert_gate_in(&state, &ls, &gates::g_k(&ls));
}
