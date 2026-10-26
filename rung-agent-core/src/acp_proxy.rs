//! `--acp` over stdio with one worker process per ACP session.
//!
//! The parent speaks JSON-RPC to the client but runs no turn itself. Each
//! session lives in its own detached child (`rung-agent --acp`, marked by
//! [`WORKER_ENV`]), so a long turn, a crash, an out-of-memory kill or a hang
//! in one session cannot hold up or take down another, and the
//! process-wide working directory a turn sets belongs to that session alone.
//!
//! Messages are routed by `params.sessionId`; lines are relayed verbatim, so
//! streaming (`session/update`) and `session/cancel` cross the process
//! boundary unchanged. A worker that dies fails the requests it had open,
//! and the next message for its session starts a fresh worker that
//! `session/resume`s from the session file. Restarts back off from 250 ms to
//! 30 s and stop after 5 consecutive failures; a worker that lived 30 s
//! counts as stable and clears the count (the same shape as prime-agent's
//! supervisor, ideas only).
//!
//! Set `RUNG_ACP_WORKERS=0` to keep the old single-process agent. `--acp-http`
//! is unchanged and stays in-process.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// Set in a worker's environment: it serves ACP itself, whole.
pub const WORKER_ENV: &str = "RUNG_ACP_WORKER";
/// `0` turns the per-session workers off.
pub const WORKERS_ENV: &str = "RUNG_ACP_WORKERS";

const MAX_FAILURES: u32 = 5;
const BACKOFF_FIRST: Duration = Duration::from_millis(250);
const BACKOFF_MAX: Duration = Duration::from_secs(30);
const STABLE: Duration = Duration::from_secs(30);
const PROXY_ID: &str = "rung-proxy:";

/// Whether this process should front per-session workers.
pub fn enabled() -> bool {
    std::env::var_os(WORKER_ENV).is_none()
        && std::env::var(WORKERS_ENV).map(|v| v != "0").unwrap_or(true)
}

/// Backoff before restart number `failures` (1-based).
pub fn backoff(failures: u32) -> Duration {
    let shift = failures.saturating_sub(1).min(16);
    (BACKOFF_FIRST * (1u32 << shift)).min(BACKOFF_MAX)
}

enum Event {
    Client(Option<String>),
    Worker(u64, Option<String>),
}

struct Worker {
    child: Child,
    stdin: Option<ChildStdin>,
    started: Instant,
    sessions: HashSet<String>,
    /// Client request ids (as sent) awaiting this worker's answer.
    pending: HashMap<String, Value>,
}

#[derive(Default)]
struct Route {
    wid: Option<u64>,
    cwd: Option<String>,
    mcp: Value,
    failures: u32,
    retry_at: Option<Instant>,
}

struct Proxy {
    exe: std::path::PathBuf,
    argv: Vec<String>,
    tx: Sender<Event>,
    next_wid: u64,
    next_proxy_id: u64,
    workers: HashMap<u64, Worker>,
    routes: HashMap<String, Route>,
    /// The `initialize` request, replayed to every worker.
    init: Option<Value>,
    /// Worker that has no session yet (the one that answered `initialize`).
    spare: Option<u64>,
    /// new/fork request id → (worker, request params), to learn the id.
    learning: HashMap<String, (u64, Value)>,
    /// Requests whose worker should be retired after the answer.
    retire: HashMap<String, (u64, String)>,
}

/// Run the proxy until the client closes stdin.
pub fn run(argv: Vec<String>) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let (tx, rx) = channel();
    let tx2 = tx.clone();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => {
                    if tx2.send(Event::Client(Some(l))).is_err() {
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx2.send(Event::Client(None));
    });
    let mut p = Proxy {
        exe,
        argv,
        tx,
        next_wid: 0,
        next_proxy_id: 0,
        workers: HashMap::new(),
        routes: HashMap::new(),
        init: None,
        spare: None,
        learning: HashMap::new(),
        retire: HashMap::new(),
    };
    while let Ok(ev) = rx.recv() {
        match ev {
            Event::Client(Some(line)) => p.client_line(&line),
            Event::Client(None) => break,
            Event::Worker(wid, Some(line)) => p.worker_line(wid, &line),
            Event::Worker(wid, None) => p.worker_gone(wid),
        }
    }
    p.shutdown();
    Ok(())
}

fn to_client(v: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

fn error_reply(id: &Value, msg: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32603, "message": msg}})
}

fn key(id: &Value) -> String {
    id.to_string()
}

impl Proxy {
    fn spawn_worker(&mut self) -> Result<u64, String> {
        let mut cmd = Command::new(&self.exe);
        cmd.args(&self.argv)
            .env(WORKER_ENV, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let mut child = cmd.spawn().map_err(|e| format!("spawn worker: {e}"))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("worker stdout")?;
        let wid = self.next_wid;
        self.next_wid += 1;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(l) => {
                        if tx.send(Event::Worker(wid, Some(l))).is_err() {
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = tx.send(Event::Worker(wid, None));
        });
        self.workers.insert(
            wid,
            Worker {
                child,
                stdin,
                started: Instant::now(),
                sessions: HashSet::new(),
                pending: HashMap::new(),
            },
        );
        if let Some(init) = self.init.clone() {
            self.proxy_request(wid, "initialize", init);
        }
        Ok(wid)
    }

    /// A request of the proxy's own; its answer is dropped.
    fn proxy_request(&mut self, wid: u64, method: &str, params: Value) {
        self.next_proxy_id += 1;
        let id = format!("{PROXY_ID}{}", self.next_proxy_id);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write_worker(wid, &msg.to_string());
    }

    fn write_worker(&mut self, wid: u64, line: &str) -> bool {
        let Some(w) = self.workers.get_mut(&wid) else {
            return false;
        };
        let Some(stdin) = w.stdin.as_mut() else {
            return false;
        };
        writeln!(stdin, "{line}")
            .and_then(|_| stdin.flush())
            .is_ok()
    }

    /// Any live worker, else a new one.
    fn any_worker(&mut self) -> Result<u64, String> {
        if let Some(w) = self.spare.filter(|w| self.workers.contains_key(w)) {
            return Ok(w);
        }
        if let Some(w) = self.workers.keys().min().copied() {
            return Ok(w);
        }
        self.spawn_worker()
    }

    /// The worker for an existing session, restarting one that died.
    fn worker_for(&mut self, sid: &str, method: &str) -> Result<u64, String> {
        let route = self.routes.entry(sid.to_string()).or_default();
        if let Some(w) = route.wid.filter(|w| self.workers.contains_key(w)) {
            return Ok(w);
        }
        if route.failures >= MAX_FAILURES {
            return Err(format!(
                "session worker failed {} times in a row; not restarting",
                route.failures
            ));
        }
        if let Some(at) = route.retry_at {
            let now = Instant::now();
            if now < at {
                return Err(format!(
                    "session worker is restarting; retry in {} ms",
                    (at - now).as_millis()
                ));
            }
        }
        let (cwd, mcp) = (route.cwd.clone(), route.mcp.clone());
        let wid = self.spawn_worker()?;
        if let Some(w) = self.workers.get_mut(&wid) {
            w.sessions.insert(sid.to_string());
        }
        if let Some(r) = self.routes.get_mut(sid) {
            r.wid = Some(wid);
        }
        // A restarted session is brought back from its file first; a load or
        // resume the client sent brings it back itself.
        if method != "session/load"
            && method != "session/resume"
            && let Some(cwd) = cwd
        {
            let mcp = if mcp.is_null() { json!([]) } else { mcp };
            self.proxy_request(
                wid,
                "session/resume",
                json!({"sessionId": sid, "cwd": cwd, "mcpServers": mcp}),
            );
        }
        Ok(wid)
    }

    fn remember(&mut self, sid: &str, wid: u64, params: &Value) {
        let route = self.routes.entry(sid.to_string()).or_default();
        route.wid = Some(wid);
        if let Some(c) = params.get("cwd").and_then(Value::as_str) {
            route.cwd = Some(c.to_string());
        }
        if let Some(m) = params.get("mcpServers") {
            route.mcp = m.clone();
        }
        if let Some(w) = self.workers.get_mut(&wid) {
            w.sessions.insert(sid.to_string());
        }
    }

    fn client_line(&mut self, line: &str) {
        if line.trim().is_empty() {
            return;
        }
        let Ok(msg) = serde_json::from_str::<Value>(line) else {
            to_client(&json!({"jsonrpc": "2.0", "id": null,
                "error": {"code": -32700, "message": "parse error"}}));
            return;
        };
        let method = msg
            .get("method")
            .and_then(Value::as_str)
            .map(str::to_string);
        let id = msg.get("id").cloned();
        let Some(method) = method else {
            // A client response: nothing the workers ask for needs one today.
            return;
        };
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let sid = params
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string);

        let target = if method == "initialize" {
            self.init = Some(params.clone());
            self.any_worker()
        } else if method == "session/new" {
            match self.spare.take().filter(|w| self.workers.contains_key(w)) {
                Some(w) => Ok(w),
                None => self.spawn_worker(),
            }
        } else if let Some(sid) = &sid {
            if self.routes.contains_key(sid) {
                self.worker_for(sid, &method)
            } else {
                // Unknown here (an earlier run's session): a worker takes it
                // up from the session file.
                self.spawn_worker()
                    .inspect(|&w| self.remember(sid, w, &params))
            }
        } else {
            self.any_worker()
        };
        let wid = match target {
            Ok(w) => w,
            Err(e) => {
                if let Some(id) = &id {
                    to_client(&error_reply(id, &e));
                }
                return;
            }
        };
        if self.spare == Some(wid) && method != "initialize" {
            self.spare = None;
        }
        if method == "initialize" {
            self.spare = Some(wid);
        }
        if let Some(id) = &id {
            let k = key(id);
            if let Some(w) = self.workers.get_mut(&wid) {
                w.pending.insert(k.clone(), id.clone());
            }
            if method == "session/new" || method == "session/fork" {
                self.learning.insert(k.clone(), (wid, params.clone()));
            }
            if matches!(method.as_str(), "session/load" | "session/resume")
                && let Some(sid) = &sid
            {
                self.remember(sid, wid, &params);
            }
            if matches!(method.as_str(), "session/close" | "session/delete")
                && let Some(sid) = &sid
            {
                self.retire.insert(k, (wid, sid.clone()));
            }
        }
        if !self.write_worker(wid, line)
            && let Some(id) = &id
        {
            self.drop_pending(wid, id);
            to_client(&error_reply(id, "session worker is not accepting input"));
        }
    }

    fn drop_pending(&mut self, wid: u64, id: &Value) {
        let k = key(id);
        if let Some(w) = self.workers.get_mut(&wid) {
            w.pending.remove(&k);
        }
        self.learning.remove(&k);
        self.retire.remove(&k);
    }

    fn worker_line(&mut self, wid: u64, line: &str) {
        let Ok(msg) = serde_json::from_str::<Value>(line) else {
            return;
        };
        let is_response = msg.get("method").is_none() && msg.get("id").is_some();
        if !is_response {
            to_client(&msg);
            return;
        }
        let id = msg["id"].clone();
        if id.as_str().is_some_and(|s| s.starts_with(PROXY_ID)) {
            return;
        }
        let k = key(&id);
        if let Some(w) = self.workers.get_mut(&wid) {
            w.pending.remove(&k);
        }
        if let Some((w, params)) = self.learning.remove(&k)
            && let Some(sid) = msg["result"].get("sessionId").and_then(Value::as_str)
        {
            self.remember(sid, w, &params);
        }
        to_client(&msg);
        if let Some((w, sid)) = self.retire.remove(&k) {
            self.retire_session(w, &sid);
        }
    }

    /// A closed or deleted session's worker goes with it, unless it holds others.
    fn retire_session(&mut self, wid: u64, sid: &str) {
        self.routes.remove(sid);
        let empty = match self.workers.get_mut(&wid) {
            Some(w) => {
                w.sessions.remove(sid);
                w.sessions.is_empty() && w.pending.is_empty()
            }
            None => false,
        };
        if empty && self.spare != Some(wid) && self.workers.len() > 1 {
            self.stop_worker(wid);
        }
    }

    fn stop_worker(&mut self, wid: u64) {
        if let Some(mut w) = self.workers.remove(&wid) {
            drop(w.stdin.take());
            std::thread::spawn(move || {
                let t = Instant::now();
                while t.elapsed() < Duration::from_secs(5) {
                    if matches!(w.child.try_wait(), Ok(Some(_))) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                let _ = w.child.kill();
                let _ = w.child.wait();
            });
        }
    }

    /// A worker's stdout ended without the proxy retiring it: it died.
    fn worker_gone(&mut self, wid: u64) {
        let Some(mut w) = self.workers.remove(&wid) else {
            return;
        };
        let _ = w.child.wait();
        let stable = w.started.elapsed() >= STABLE;
        for (k, id) in &w.pending {
            self.learning.remove(k);
            self.retire.remove(k);
            to_client(&error_reply(id, "session worker exited"));
        }
        if self.spare == Some(wid) {
            self.spare = None;
        }
        for sid in &w.sessions {
            if let Some(r) = self.routes.get_mut(sid) {
                r.wid = None;
                r.failures = if stable { 1 } else { r.failures + 1 };
                r.retry_at = Some(Instant::now() + backoff(r.failures));
            }
        }
    }

    fn shutdown(&mut self) {
        let ids: Vec<u64> = self.workers.keys().copied().collect();
        for wid in ids {
            if let Some(w) = self.workers.get_mut(&wid) {
                drop(w.stdin.take());
            }
        }
        let t = Instant::now();
        for w in self.workers.values_mut() {
            while t.elapsed() < Duration::from_secs(5) {
                if matches!(w.child.try_wait(), Ok(Some(_))) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            let _ = w.child.kill();
            let _ = w.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_to_the_cap() {
        assert_eq!(backoff(1), Duration::from_millis(250));
        assert_eq!(backoff(2), Duration::from_millis(500));
        assert_eq!(backoff(5), Duration::from_secs(4));
        assert_eq!(backoff(40), BACKOFF_MAX);
    }
}
