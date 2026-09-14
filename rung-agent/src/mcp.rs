//! MCP client (stdio + streamable HTTP). Product, not kernel.
//!
//! Discovers remote tools and admits them as a [`Toolset`]. Harbor hello-mcp
//! is streamable HTTP; ACP requires stdio.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use rung_std::llm::ToolDefinition;
use rung_std::tools::Toolset;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
pub const MAX_ATTEMPTS: u32 = 2;

thread_local! {
    static SESSION_CANCEL: std::cell::RefCell<Option<Arc<AtomicBool>>> = const { std::cell::RefCell::new(None) };
    static SESSION_SINK: std::cell::RefCell<Option<(PathBuf, String)>> = const { std::cell::RefCell::new(None) };
    static DYNAMIC_SECRETS: std::cell::RefCell<HashSet<String>> = std::cell::RefCell::new(HashSet::new());
}

pub struct CancelScopeGuard;

impl Drop for CancelScopeGuard {
    fn drop(&mut self) {
        SESSION_CANCEL.with(|c| *c.borrow_mut() = None);
    }
}

pub fn set_session_cancel(cancel: Option<Arc<AtomicBool>>) -> CancelScopeGuard {
    SESSION_CANCEL.with(|c| *c.borrow_mut() = cancel);
    CancelScopeGuard
}

pub fn is_session_cancelled() -> bool {
    SESSION_CANCEL.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    })
}

pub struct SessionSinkGuard;

impl Drop for SessionSinkGuard {
    fn drop(&mut self) {
        SESSION_SINK.with(|s| *s.borrow_mut() = None);
    }
}

pub fn set_session_sink(sink: Option<(PathBuf, String)>) -> SessionSinkGuard {
    SESSION_SINK.with(|s| *s.borrow_mut() = sink);
    SessionSinkGuard
}

pub fn register_secret(val: &str) {
    let s = val.trim();
    if s.len() >= 4 {
        DYNAMIC_SECRETS.with(|set| {
            set.borrow_mut().insert(s.to_string());
        });
    }
}

fn record_session_start(tool: &str, rpc_id: u64, op_id: Option<&str>) {
    SESSION_SINK.with(|sink| {
        if let Some((dir, sid)) = sink.borrow().as_ref() {
            let file = dir.join(format!("{sid}.mcp.log"));
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(file)
            {
                let _ = writeln!(
                    f,
                    "{{\"event\":\"start\",\"tool\":\"{tool}\",\"rpc_id\":{rpc_id},\"operation_id\":{}}}",
                    op_id.map_or("null".to_string(), |id| format!("\"{id}\""))
                );
            }
        }
    });
}

fn record_session_error(err_json: &str) {
    SESSION_SINK.with(|sink| {
        if let Some((dir, sid)) = sink.borrow().as_ref() {
            let file = dir.join(format!("{sid}.mcp.log"));
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(file)
            {
                let _ = writeln!(f, "{err_json}");
            }
        }
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolError {
    pub layer: String,
    pub method: String,
    pub tool: String,
    pub request_id: Option<Value>,
    pub operation_id: Option<String>,
    pub retryable: bool,
    pub outcome: String,
    pub corrective_action: String,
    pub cause_chain: Vec<String>,
}

impl McpToolError {
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!(
                r#"{{"layer":"{}","method":"{}","tool":"{}","request_id":null,"operation_id":null,"retryable":false,"outcome":"unknown","corrective_action":"serialization error","cause_chain":[]}}"#,
                self.layer, self.method, self.tool
            )
        })
    }
}

pub fn redact(text: &str) -> String {
    let mut out = redact_url_credentials(text);
    out = redact_tokens_and_headers(&out);

    DYNAMIC_SECRETS.with(|set| {
        for secret in set.borrow().iter() {
            if !secret.is_empty() {
                out = out.replace(secret, "[REDACTED]");
            }
        }
    });

    const SECRET_ENVS: &[&str] = &[
        "OPENROUTER_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "RUNG_API_KEY",
        "HOST_TOKEN",
        "HOST_VENUE_KEY",
        "XAI_API_KEY",
    ];
    for key in SECRET_ENVS {
        if let Ok(val) = std::env::var(key) {
            let val = val.trim();
            if val.len() >= 6 {
                out = out.replace(val, "[REDACTED]");
            }
        }
    }

    out
}

fn redact_url_credentials(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for (idx, _) in s.match_indices("://") {
        if idx < last {
            continue;
        }
        let after_scheme = idx + 3;
        let auth_end = s[after_scheme..]
            .char_indices()
            .find(|&(_, c)| {
                c == '/'
                    || c == '?'
                    || c == '#'
                    || c.is_whitespace()
                    || c == '"'
                    || c == '\''
                    || c == ')'
            })
            .map(|(i, _)| after_scheme + i)
            .unwrap_or(s.len());

        let authority = &s[after_scheme..auth_end];
        if let Some(at_idx) = authority.find('@') {
            let user_info = &authority[..at_idx];
            let host_part = &authority[at_idx..];
            out.push_str(&s[last..after_scheme]);
            if let Some(colon_idx) = user_info.find(':') {
                let user = &user_info[..colon_idx];
                out.push_str(user);
                out.push_str(":[REDACTED]");
            } else {
                out.push_str("[REDACTED]");
            }
            out.push_str(host_part);
            last = auth_end;
        }
    }
    out.push_str(&s[last..]);
    out
}

fn starts_with_ascii_ignore_case(slice: &str, pat: &str) -> bool {
    if slice.len() < pat.len() {
        false
    } else {
        slice.as_bytes()[..pat.len()].eq_ignore_ascii_case(pat.as_bytes())
    }
}

fn redact_tokens_and_headers(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;

    while i < s.len() {
        let remainder = &s[i..];

        // Check "bearer "
        if starts_with_ascii_ignore_case(remainder, "bearer ") {
            out.push_str(&remainder[..7]);
            let mut val_idx = i + 7;
            while val_idx < s.len()
                && (s.as_bytes()[val_idx] == b' ' || s.as_bytes()[val_idx] == b'\t')
            {
                out.push(s.as_bytes()[val_idx] as char);
                val_idx += 1;
            }
            let mut end = val_idx;
            while end < s.len() {
                let b = s.as_bytes()[end];
                if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.' {
                    end += 1;
                } else {
                    break;
                }
            }
            if end - val_idx >= 6 {
                out.push_str("[REDACTED]");
                i = end;
                continue;
            } else {
                out.push_str(&s[val_idx..end]);
                i = end;
                continue;
            }
        }

        // Check header prefixes
        let mut matched_header = None;
        for &hdr in &["authorization:", "x-api-key:", "mcp-session-id:"] {
            if starts_with_ascii_ignore_case(remainder, hdr) {
                matched_header = Some(hdr);
                break;
            }
        }

        if let Some(hdr) = matched_header {
            out.push_str(&remainder[..hdr.len()]);
            let mut val_idx = i + hdr.len();
            while val_idx < s.len()
                && (s.as_bytes()[val_idx] == b' ' || s.as_bytes()[val_idx] == b'\t')
            {
                out.push(s.as_bytes()[val_idx] as char);
                val_idx += 1;
            }
            let mut end = val_idx;
            while end < s.len() {
                let b = s.as_bytes()[end];
                if b == b'\r' || b == b'\n' || b == b',' || b == b';' || b == b'"' || b == b'\'' {
                    break;
                }
                let ch_len = s[end..].chars().next().map_or(1, |c| c.len_utf8());
                end += ch_len;
            }
            if end > val_idx {
                out.push_str("[REDACTED]");
                i = end;
                continue;
            }
        }

        // Advance by one UTF-8 character
        let ch = remainder.chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

fn classify_reqwest_error(
    e: &reqwest::Error,
    tool: &str,
    request_id: u64,
    operation_id: Option<&str>,
    retryable: bool,
) -> McpToolError {
    let is_timeout = e.is_timeout();
    let is_connect = e.is_connect();
    let is_body = e.is_body();

    let (classification, corrective_action) = if is_timeout {
        (
            "timeout",
            "Request timed out within remaining budget. Verify server performance or retry with the same operation_id.",
        )
    } else if is_connect {
        (
            "connect",
            "Failed to connect to MCP server. Verify server address and availability or retry with the same operation_id.",
        )
    } else if is_body {
        (
            "body",
            "Transport body error during transfer. Retry with the same operation_id if idempotent; do not submit with a new operation_id.",
        )
    } else {
        (
            "transport",
            "Transport error sending request to MCP server. Verify connectivity or retry with the same operation_id.",
        )
    };

    let mut cause_chain = Vec::new();
    cause_chain.push(redact(&format!("{classification}: {e}")));
    let mut curr = (e as &dyn std::error::Error).source();
    while let Some(src) = curr {
        cause_chain.push(redact(&src.to_string()));
        curr = src.source();
    }

    McpToolError {
        layer: "mcp".into(),
        method: "tools/call".into(),
        tool: tool.to_string(),
        request_id: Some(json!(request_id)),
        operation_id: operation_id.map(str::to_string),
        retryable,
        outcome: "unknown".into(),
        corrective_action: corrective_action.into(),
        cause_chain,
    }
}

#[derive(Debug, Clone)]
pub enum McpSpec {
    Http {
        name: String,
        url: String,
        headers: Vec<(String, String)>,
    },
    Stdio {
        name: String,
        command: PathBuf,
        args: Vec<String>,
        env: Vec<(String, String)>,
    },
}

impl McpSpec {
    pub fn name(&self) -> &str {
        match self {
            Self::Http { name, .. } | Self::Stdio { name, .. } => name,
        }
    }

    /// `name=url` for `--mcp-http`.
    pub fn parse_http(spec: &str) -> Result<Self, String> {
        let (name, url) = spec
            .split_once('=')
            .ok_or_else(|| "--mcp-http wants name=url".to_string())?;
        let name = name.trim();
        let url = url.trim();
        if name.is_empty() || url.is_empty() {
            return Err("--mcp-http wants name=url".into());
        }
        Ok(Self::Http {
            name: name.into(),
            url: url.into(),
            headers: Vec::new(),
        })
    }
}

trait Wire: Send + Sync {
    fn rpc(&self, method: &str, params: Value, notification: bool) -> Result<Value, String>;

    fn call_tool(
        &self,
        tool_name: &str,
        remote_name: &str,
        args: &Value,
        op_id: Option<&str>,
        can_retry: bool,
    ) -> Result<Value, String>;

    fn set_cancel(&self, cancel: Option<Arc<AtomicBool>>);
}

pub struct HttpWire {
    url: String,
    blocking_client: reqwest::blocking::Client,
    async_client: reqwest::Client,
    extra: HeaderMap,
    session: Mutex<Option<String>>,
    next_id: AtomicU64,
    timeout: Duration,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
}

impl HttpWire {
    pub const DEFAULT_TIMEOUT: Duration = DEFAULT_TIMEOUT;

    pub fn new(url: String, headers: &[(String, String)]) -> Result<Self, String> {
        Self::new_with_timeout(url, headers, DEFAULT_TIMEOUT)
    }

    pub fn new_with_timeout(
        url: String,
        headers: &[(String, String)],
        timeout: Duration,
    ) -> Result<Self, String> {
        let mut extra = HeaderMap::new();
        for (k, v) in headers {
            let name =
                HeaderName::from_bytes(k.as_bytes()).map_err(|e| format!("mcp header {k}: {e}"))?;
            let val = HeaderValue::from_str(v).map_err(|e| format!("mcp header {k}: {e}"))?;
            extra.insert(name, val);
            register_secret(v);
        }
        let blocking_client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;

        let async_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            url,
            blocking_client,
            async_client,
            extra,
            session: Mutex::new(None),
            next_id: AtomicU64::new(1),
            timeout,
            cancel: Mutex::new(None),
        })
    }

    fn is_cancelled(&self) -> bool {
        self.cancel
            .lock()
            .expect("mcp cancel lock")
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }
}

fn run_async<F, R>(f: F) -> Result<R, String>
where
    F: std::future::Future<Output = Result<R, String>>,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| handle.block_on(f))
            }
            _ => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("tokio runtime: {e}"))?;
                rt.block_on(f)
            }
        },
        Err(_) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio runtime: {e}"))?;
            rt.block_on(f)
        }
    }
}

impl Wire for HttpWire {
    fn set_cancel(&self, cancel: Option<Arc<AtomicBool>>) {
        *self.cancel.lock().expect("mcp cancel lock") = cancel;
    }

    fn rpc(&self, method: &str, params: Value, notification: bool) -> Result<Value, String> {
        if self.is_cancelled() || is_session_cancelled() {
            return Err("mcp: execution cancelled".into());
        }
        let mut body = json!({"jsonrpc": "2.0", "method": method, "params": params});
        if !notification {
            body["id"] = json!(self.next_id.fetch_add(1, Ordering::Relaxed));
        }
        let mut req = self
            .blocking_client
            .post(&self.url)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2024-11-05")
            .headers(self.extra.clone());
        if let Some(sid) = self.session.lock().expect("mcp session").as_ref() {
            req = req.header("Mcp-Session-Id", sid.clone());
        }
        let resp = req
            .json(&body)
            .send()
            .map_err(|e| format!("mcp http: {}", redact(&e.to_string())))?;
        if let Some(sid) = resp
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            *self.session.lock().expect("mcp session") = Some(sid.to_string());
            register_secret(sid);
        }
        let status = resp.status();
        let text = resp
            .text()
            .map_err(|e| format!("mcp http body: {}", redact(&e.to_string())))?;
        if !status.is_success() {
            return Err(format!("mcp http {status}: {}", redact(&text)));
        }
        if notification {
            return Ok(Value::Null);
        }
        let parsed = parse_rpc_body(&text)?;
        if let Some(err) = parsed.get("error") {
            return Err(format!("mcp {method}: {}", redact(&err.to_string())));
        }
        Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
    }

    fn call_tool(
        &self,
        tool_name: &str,
        remote_name: &str,
        args: &Value,
        op_id: Option<&str>,
        can_retry: bool,
    ) -> Result<Value, String> {
        let rpc_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let body = json!({
            "jsonrpc": "2.0",
            "id": rpc_id,
            "method": "tools/call",
            "params": {
                "name": remote_name,
                "arguments": args,
            }
        });

        // Dedicated operation-start log BEFORE FIRST SEND
        eprintln!(
            "[mcp] operation-start tool={tool_name} rpc_id={rpc_id} operation_id={}",
            op_id.unwrap_or("none")
        );
        record_session_start(tool_name, rpc_id, op_id);

        let max_attempts = if can_retry { MAX_ATTEMPTS } else { 1 };
        let start_time = Instant::now();
        let total_timeout = self.timeout;
        let url = self.url.clone();
        let client = self.async_client.clone();
        let extra = self.extra.clone();
        let session_mutex = &self.session;
        let cancel_arc = self.cancel.lock().expect("mcp cancel").clone();
        let tool_name_owned = tool_name.to_string();
        let op_id_owned = op_id.map(str::to_string);

        run_async(async move {
            let mut attempt = 0;
            loop {
                attempt += 1;

                if cancel_arc
                    .as_ref()
                    .is_some_and(|f| f.load(Ordering::SeqCst))
                    || is_session_cancelled()
                {
                    let err = McpToolError {
                        layer: "mcp".into(),
                        method: "tools/call".into(),
                        tool: tool_name_owned.clone(),
                        request_id: Some(json!(rpc_id)),
                        operation_id: op_id_owned.clone(),
                        retryable: false,
                        outcome: "unknown".into(),
                        corrective_action: "Execution was cancelled. Outcome is unknown; do not retry with a new operation_id.".into(),
                        cause_chain: vec!["cancelled: execution cancelled".into()],
                    };
                    let err_str = err.to_json_string();
                    eprintln!("[mcp] error: {}", redact(&err_str));
                    record_session_error(&err_str);
                    return Err(err_str);
                }

                let elapsed = start_time.elapsed();
                if elapsed >= total_timeout {
                    let err = McpToolError {
                        layer: "mcp".into(),
                        method: "tools/call".into(),
                        tool: tool_name_owned.clone(),
                        request_id: Some(json!(rpc_id)),
                        operation_id: op_id_owned.clone(),
                        retryable: can_retry,
                        outcome: "unknown".into(),
                        corrective_action: "Total timeout budget expired. Verify server performance or retry with the same operation_id.".into(),
                        cause_chain: vec!["timeout: total budget expired".into()],
                    };
                    let err_str = err.to_json_string();
                    eprintln!("[mcp] error: {}", redact(&err_str));
                    record_session_error(&err_str);
                    return Err(err_str);
                }

                let remaining_budget = total_timeout - elapsed;

                let mut req = client
                    .post(&url)
                    .timeout(remaining_budget)
                    .header(CONTENT_TYPE, "application/json")
                    .header(ACCEPT, "application/json, text/event-stream")
                    .header("MCP-Protocol-Version", "2024-11-05")
                    .headers(extra.clone());
                if let Some(sid) = session_mutex.lock().expect("mcp session").as_ref() {
                    req = req.header("Mcp-Session-Id", sid.clone());
                }

                let send_req = req.json(&body);

                let cancel_watcher = {
                    let flag = cancel_arc.clone();
                    async move {
                        loop {
                            if flag.as_ref().is_some_and(|f| f.load(Ordering::SeqCst))
                                || is_session_cancelled()
                            {
                                return;
                            }
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                };

                let network_fut = async {
                    let resp = send_req.send().await?;
                    let headers = resp.headers().clone();
                    let status = resp.status();
                    let body_text = resp.text().await?;
                    Ok::<_, reqwest::Error>((headers, status, body_text))
                };

                let res = tokio::select! {
                    _ = cancel_watcher => {
                        let err = McpToolError {
                            layer: "mcp".into(),
                            method: "tools/call".into(),
                            tool: tool_name_owned.clone(),
                            request_id: Some(json!(rpc_id)),
                            operation_id: op_id_owned.clone(),
                            retryable: false,
                            outcome: "unknown".into(),
                            corrective_action: "Execution was cancelled. Outcome is unknown; do not retry with a new operation_id.".into(),
                            cause_chain: vec!["cancelled: execution cancelled in-flight".into()],
                        };
                        let err_str = err.to_json_string();
                        eprintln!("[mcp] error: {}", redact(&err_str));
                        record_session_error(&err_str);
                        return Err(err_str);
                    }
                    r = network_fut => r,
                };

                let (headers, status, body_text) = match res {
                    Ok(data) => data,
                    Err(e) => {
                        if can_retry
                            && attempt < max_attempts
                            && !cancel_arc
                                .as_ref()
                                .is_some_and(|f| f.load(Ordering::SeqCst))
                            && !is_session_cancelled()
                            && start_time.elapsed() < total_timeout
                        {
                            continue;
                        }

                        let err = classify_reqwest_error(
                            &e,
                            &tool_name_owned,
                            rpc_id,
                            op_id_owned.as_deref(),
                            can_retry,
                        );
                        let err_str = err.to_json_string();
                        eprintln!("[mcp] error: {}", redact(&err_str));
                        record_session_error(&err_str);
                        return Err(err_str);
                    }
                };

                if let Some(sid) = headers.get("mcp-session-id").and_then(|v| v.to_str().ok()) {
                    *session_mutex.lock().expect("mcp session") = Some(sid.to_string());
                    register_secret(sid);
                }

                if !status.is_success() {
                    let redacted_body = redact(&body_text);
                    let err = McpToolError {
                        layer: "mcp".into(),
                        method: "tools/call".into(),
                        tool: tool_name_owned.clone(),
                        request_id: Some(json!(rpc_id)),
                        operation_id: op_id_owned.clone(),
                        retryable: false,
                        outcome: "unknown".into(),
                        corrective_action: format!(
                            "Server returned HTTP status {status}. Inspect server logs; do not resubmit with a new operation_id until status is verified."
                        ),
                        cause_chain: vec![format!("http {status}: {redacted_body}")],
                    };
                    let err_str = err.to_json_string();
                    eprintln!("[mcp] error: {}", redact(&err_str));
                    record_session_error(&err_str);
                    return Err(err_str);
                }

                let parsed = match parse_rpc_body(&body_text) {
                    Ok(v) => v,
                    Err(e) => {
                        let err = McpToolError {
                            layer: "mcp".into(),
                            method: "tools/call".into(),
                            tool: tool_name_owned.clone(),
                            request_id: Some(json!(rpc_id)),
                            operation_id: op_id_owned.clone(),
                            retryable: false,
                            outcome: "unknown".into(),
                            corrective_action: "Failed to parse JSON-RPC response from server."
                                .into(),
                            cause_chain: vec![redact(&e)],
                        };
                        let err_str = err.to_json_string();
                        eprintln!("[mcp] error: {}", redact(&err_str));
                        record_session_error(&err_str);
                        return Err(err_str);
                    }
                };

                if parsed.get("id") != Some(&json!(rpc_id)) {
                    let err = McpToolError {
                        layer: "mcp".into(),
                        method: "tools/call".into(),
                        tool: tool_name_owned.clone(),
                        request_id: Some(json!(rpc_id)),
                        operation_id: op_id_owned.clone(),
                        retryable: false,
                        outcome: "unknown".into(),
                        corrective_action:
                            "JSON-RPC response id mismatch. Server response cannot be correlated."
                                .into(),
                        cause_chain: vec![format!(
                            "mcp: response id {:?} does not match request id {rpc_id}",
                            parsed.get("id")
                        )],
                    };
                    let err_str = err.to_json_string();
                    eprintln!("[mcp] error: {}", redact(&err_str));
                    record_session_error(&err_str);
                    return Err(err_str);
                }

                if let Some(err_val) = parsed.get("error") {
                    let err = McpToolError {
                        layer: "mcp".into(),
                        method: "tools/call".into(),
                        tool: tool_name_owned.clone(),
                        request_id: Some(json!(rpc_id)),
                        operation_id: op_id_owned.clone(),
                        retryable: false,
                        outcome: "unknown".into(),
                        corrective_action: "JSON-RPC error returned by MCP server.".into(),
                        cause_chain: vec![redact(&err_val.to_string())],
                    };
                    let err_str = err.to_json_string();
                    eprintln!("[mcp] error: {}", redact(&err_str));
                    record_session_error(&err_str);
                    return Err(err_str);
                }

                let Some(result_obj) = parsed.get("result").and_then(|r| r.as_object()) else {
                    let err = McpToolError {
                        layer: "mcp".into(),
                        method: "tools/call".into(),
                        tool: tool_name_owned.clone(),
                        request_id: Some(json!(rpc_id)),
                        operation_id: op_id_owned.clone(),
                        retryable: false,
                        outcome: "unknown".into(),
                        corrective_action: "Malformed JSON-RPC response: expected result object."
                            .into(),
                        cause_chain: vec!["mcp: response missing result object".into()],
                    };
                    let err_str = err.to_json_string();
                    eprintln!("[mcp] error: {}", redact(&err_str));
                    record_session_error(&err_str);
                    return Err(err_str);
                };

                let result = Value::Object(result_obj.clone());

                let is_error = result
                    .get("isError")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if is_error {
                    let text = content_text(&result);
                    let redacted_text = redact(&text);

                    let mut corrective_action =
                        "Inspect tool error response and adjust input.".to_string();
                    let mut retryable = false;
                    let mut cause_chain = Vec::new();

                    cause_chain.push(redacted_text.clone());

                    if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(&text) {
                        if let Some(ca) = obj.get("corrective_action").and_then(|v| v.as_str()) {
                            corrective_action = ca.to_string();
                        }
                        if let Some(r) = obj.get("retryable").and_then(|v| v.as_bool()) {
                            retryable = r;
                        }
                        if let Some(errors) = obj.get("errors").and_then(|v| v.as_array()) {
                            for err_item in errors {
                                if let Some(msg) = err_item.as_str() {
                                    cause_chain.push(redact(msg));
                                }
                            }
                        }
                        if let Some(diag) = obj.get("diagnostic") {
                            cause_chain.push(redact(&diag.to_string()));
                        }
                    }

                    let err = McpToolError {
                        layer: "mcp".into(),
                        method: "tools/call".into(),
                        tool: tool_name_owned.clone(),
                        request_id: Some(json!(rpc_id)),
                        operation_id: op_id_owned.clone(),
                        retryable,
                        outcome: "unknown".into(),
                        corrective_action,
                        cause_chain,
                    };
                    let err_str = err.to_json_string();
                    eprintln!("[mcp] error: {}", redact(&err_str));
                    record_session_error(&err_str);
                    return Err(err_str);
                }

                if result.get("content").and_then(|c| c.as_array()).is_none() {
                    let err = McpToolError {
                        layer: "mcp".into(),
                        method: "tools/call".into(),
                        tool: tool_name_owned.clone(),
                        request_id: Some(json!(rpc_id)),
                        operation_id: op_id_owned.clone(),
                        retryable: false,
                        outcome: "unknown".into(),
                        corrective_action: "Malformed JSON-RPC result: missing content array."
                            .into(),
                        cause_chain: vec!["mcp: result object missing content array".into()],
                    };
                    let err_str = err.to_json_string();
                    eprintln!("[mcp] error: {}", redact(&err_str));
                    record_session_error(&err_str);
                    return Err(err_str);
                }

                return Ok(result);
            }
        })
    }
}

struct StdioWire {
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    next_id: AtomicU64,
    _child: Mutex<Child>,
}

impl StdioWire {
    fn spawn(spec: &McpSpec) -> Result<Self, String> {
        let McpSpec::Stdio {
            command,
            args,
            env,
            name,
        } = spec
        else {
            return Err("stdio spec required".into());
        };
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().map_err(|e| format!("mcp stdio {name}: {e}"))?;
        let stdin = child.stdin.take().ok_or("mcp stdio: no stdin")?;
        let stdout = child.stdout.take().ok_or("mcp stdio: no stdout")?;
        Ok(Self {
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            next_id: AtomicU64::new(1),
            _child: Mutex::new(child),
        })
    }
}

impl Wire for StdioWire {
    fn set_cancel(&self, _cancel: Option<Arc<AtomicBool>>) {}

    fn rpc(&self, method: &str, params: Value, notification: bool) -> Result<Value, String> {
        if is_session_cancelled() {
            return Err("mcp: execution cancelled".into());
        }
        let mut body = json!({"jsonrpc": "2.0", "method": method, "params": params});
        if !notification {
            body["id"] = json!(self.next_id.fetch_add(1, Ordering::Relaxed));
        }
        let line = serde_json::to_string(&body).map_err(|e| e.to_string())?;
        {
            let mut stdin = self.stdin.lock().expect("mcp stdin");
            writeln!(stdin, "{line}").map_err(|e| format!("mcp stdio write: {e}"))?;
            stdin.flush().map_err(|e| format!("mcp stdio flush: {e}"))?;
        }
        if notification {
            return Ok(Value::Null);
        }
        let mut stdout = self.stdout.lock().expect("mcp stdout");
        let mut got = String::new();
        stdout
            .read_line(&mut got)
            .map_err(|e| format!("mcp stdio read: {e}"))?;
        let parsed = serde_json::from_str::<Value>(got.trim())
            .map_err(|e| format!("mcp stdio json: {e}: {}", redact(&got)))?;
        if let Some(err) = parsed.get("error") {
            return Err(format!("mcp {method}: {}", redact(&err.to_string())));
        }
        Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
    }

    fn call_tool(
        &self,
        tool_name: &str,
        remote_name: &str,
        args: &Value,
        op_id: Option<&str>,
        _can_retry: bool,
    ) -> Result<Value, String> {
        if is_session_cancelled() {
            let err = McpToolError {
                layer: "mcp".into(),
                method: "tools/call".into(),
                tool: tool_name.to_string(),
                request_id: None,
                operation_id: op_id.map(str::to_string),
                retryable: false,
                outcome: "unknown".into(),
                corrective_action: "Execution was cancelled. Outcome is unknown; do not retry with a new operation_id.".into(),
                cause_chain: vec!["cancelled: execution cancelled".into()],
            };
            return Err(err.to_json_string());
        }

        let rpc_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let body = json!({
            "jsonrpc": "2.0",
            "id": rpc_id,
            "method": "tools/call",
            "params": {
                "name": remote_name,
                "arguments": args,
            }
        });

        eprintln!(
            "[mcp] operation-start tool={tool_name} rpc_id={rpc_id} operation_id={}",
            op_id.unwrap_or("none")
        );
        record_session_start(tool_name, rpc_id, op_id);

        let line = match serde_json::to_string(&body) {
            Ok(l) => l,
            Err(e) => {
                let err = McpToolError {
                    layer: "mcp".into(),
                    method: "tools/call".into(),
                    tool: tool_name.to_string(),
                    request_id: Some(json!(rpc_id)),
                    operation_id: op_id.map(str::to_string),
                    retryable: false,
                    outcome: "unknown".into(),
                    corrective_action: "Failed to serialize arguments.".into(),
                    cause_chain: vec![e.to_string()],
                };
                return Err(err.to_json_string());
            }
        };
        {
            let mut stdin = self.stdin.lock().expect("mcp stdin");
            if let Err(e) = writeln!(stdin, "{line}").and_then(|_| stdin.flush()) {
                let err = McpToolError {
                    layer: "mcp".into(),
                    method: "tools/call".into(),
                    tool: tool_name.to_string(),
                    request_id: Some(json!(rpc_id)),
                    operation_id: op_id.map(str::to_string),
                    retryable: false,
                    outcome: "unknown".into(),
                    corrective_action: "Failed to write to stdio child process.".into(),
                    cause_chain: vec![format!("mcp stdio write: {e}")],
                };
                return Err(err.to_json_string());
            }
        }
        let mut got = String::new();
        let read_res = {
            let mut stdout = self.stdout.lock().expect("mcp stdout");
            stdout.read_line(&mut got)
        };
        if let Err(e) = read_res {
            let err = McpToolError {
                layer: "mcp".into(),
                method: "tools/call".into(),
                tool: tool_name.to_string(),
                request_id: Some(json!(rpc_id)),
                operation_id: op_id.map(str::to_string),
                retryable: false,
                outcome: "unknown".into(),
                corrective_action: "Failed to read from stdio child process.".into(),
                cause_chain: vec![format!("mcp stdio read: {e}")],
            };
            return Err(err.to_json_string());
        }
        let parsed = match serde_json::from_str::<Value>(got.trim()) {
            Ok(v) => v,
            Err(e) => {
                let err = McpToolError {
                    layer: "mcp".into(),
                    method: "tools/call".into(),
                    tool: tool_name.to_string(),
                    request_id: Some(json!(rpc_id)),
                    operation_id: op_id.map(str::to_string),
                    retryable: false,
                    outcome: "unknown".into(),
                    corrective_action: "Failed to parse JSON response from stdio child process."
                        .into(),
                    cause_chain: vec![format!("mcp stdio json: {e}: {}", redact(&got))],
                };
                return Err(err.to_json_string());
            }
        };

        if parsed.get("id") != Some(&json!(rpc_id)) {
            let err = McpToolError {
                layer: "mcp".into(),
                method: "tools/call".into(),
                tool: tool_name.to_string(),
                request_id: Some(json!(rpc_id)),
                operation_id: op_id.map(str::to_string),
                retryable: false,
                outcome: "unknown".into(),
                corrective_action:
                    "JSON-RPC response id mismatch. Server response cannot be correlated.".into(),
                cause_chain: vec![format!(
                    "mcp: response id {:?} does not match request id {rpc_id}",
                    parsed.get("id")
                )],
            };
            return Err(err.to_json_string());
        }

        if let Some(err_val) = parsed.get("error") {
            let err = McpToolError {
                layer: "mcp".into(),
                method: "tools/call".into(),
                tool: tool_name.to_string(),
                request_id: Some(json!(rpc_id)),
                operation_id: op_id.map(str::to_string),
                retryable: false,
                outcome: "unknown".into(),
                corrective_action: "JSON-RPC error returned by stdio server.".into(),
                cause_chain: vec![redact(&err_val.to_string())],
            };
            return Err(err.to_json_string());
        }

        let Some(result_obj) = parsed.get("result").and_then(|r| r.as_object()) else {
            let err = McpToolError {
                layer: "mcp".into(),
                method: "tools/call".into(),
                tool: tool_name.to_string(),
                request_id: Some(json!(rpc_id)),
                operation_id: op_id.map(str::to_string),
                retryable: false,
                outcome: "unknown".into(),
                corrective_action: "Malformed JSON-RPC response: expected result object.".into(),
                cause_chain: vec!["mcp: response missing result object".into()],
            };
            return Err(err.to_json_string());
        };

        let result = Value::Object(result_obj.clone());

        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_error {
            let text = content_text(&result);
            let redacted_text = redact(&text);

            let mut corrective_action = "Inspect tool error response and adjust input.".to_string();
            let mut retryable = false;
            let mut cause_chain = vec![redacted_text.clone()];

            if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(&text) {
                if let Some(ca) = obj.get("corrective_action").and_then(|v| v.as_str()) {
                    corrective_action = ca.to_string();
                }
                if let Some(r) = obj.get("retryable").and_then(|v| v.as_bool()) {
                    retryable = r;
                }
                if let Some(errors) = obj.get("errors").and_then(|v| v.as_array()) {
                    for err_item in errors {
                        if let Some(msg) = err_item.as_str() {
                            cause_chain.push(redact(msg));
                        }
                    }
                }
                if let Some(diag) = obj.get("diagnostic") {
                    cause_chain.push(redact(&diag.to_string()));
                }
            }

            let err = McpToolError {
                layer: "mcp".into(),
                method: "tools/call".into(),
                tool: tool_name.to_string(),
                request_id: Some(json!(rpc_id)),
                operation_id: op_id.map(str::to_string),
                retryable,
                outcome: "unknown".into(),
                corrective_action,
                cause_chain,
            };
            return Err(err.to_json_string());
        }

        if result.get("content").and_then(|c| c.as_array()).is_none() {
            let err = McpToolError {
                layer: "mcp".into(),
                method: "tools/call".into(),
                tool: tool_name.to_string(),
                request_id: Some(json!(rpc_id)),
                operation_id: op_id.map(str::to_string),
                retryable: false,
                outcome: "unknown".into(),
                corrective_action: "Malformed JSON-RPC result: missing content array.".into(),
                cause_chain: vec!["mcp: result object missing content array".into()],
            };
            return Err(err.to_json_string());
        }

        Ok(result)
    }
}

pub fn parse_rpc_body(text: &str) -> Result<Value, String> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') {
        return serde_json::from_str(trimmed).map_err(|e| format!("mcp json: {e}"));
    }
    for line in trimmed.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            if data.starts_with('{') {
                return serde_json::from_str(data).map_err(|e| format!("mcp sse: {e}"));
            }
        }
    }
    Err(format!("mcp: no json in body ({})", redact(trimmed)))
}

struct RemoteTool {
    name: String,
    remote: String,
    description: String,
    input_schema: Value,
    idempotent_hint: bool,
    supports_operation_id: bool,
    wire: Arc<dyn Wire>,
}

pub struct McpRoster {
    tools: Vec<RemoteTool>,
}

impl std::fmt::Debug for McpRoster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpRoster")
            .field(
                "tools",
                &self
                    .tools
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl McpRoster {
    pub fn connect(specs: &[McpSpec]) -> Result<Self, String> {
        Self::connect_with_cancel(specs, None)
    }

    pub fn connect_with_cancel(
        specs: &[McpSpec],
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<Self, String> {
        let mut tools = Vec::new();
        let mut seen: HashMap<String, u32> = HashMap::new();
        for spec in specs {
            let wire: Arc<dyn Wire> = match spec {
                McpSpec::Http { url, headers, .. } => {
                    let w = Arc::new(HttpWire::new(url.clone(), headers)?);
                    w.set_cancel(cancel.clone());
                    w
                }
                McpSpec::Stdio { .. } => Arc::new(StdioWire::spawn(spec)?),
            };
            handshake(&*wire, spec.name())?;
            let listed = wire.rpc("tools/list", json!({}), false)?;
            let arr = listed
                .get("tools")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            for t in arr {
                let raw = t
                    .get("name")
                    .and_then(|n| n.as_str())
                    .ok_or("mcp tool missing name")?
                    .to_string();
                let n = seen.entry(raw.clone()).or_insert(0);
                *n += 1;
                let name = if *n == 1 {
                    raw.clone()
                } else {
                    format!("{}__{raw}", spec.name())
                };
                let description = t
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                let input_schema = t
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object"}));

                // Tool is idempotent ONLY if annotations declares idempotentHint: true
                let annotations = t.get("annotations");
                let idempotent_hint = annotations
                    .and_then(|a| a.get("idempotentHint"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // Tool supports operation_id ONLY if inputSchema properties defines operation_id as string
                let supports_operation_id = input_schema
                    .get("properties")
                    .and_then(|p| p.get("operation_id"))
                    .is_some_and(|op| {
                        op.get("type").and_then(|t| t.as_str()) == Some("string") || op.is_object()
                    });

                tools.push(RemoteTool {
                    name,
                    remote: raw,
                    description,
                    input_schema,
                    idempotent_hint,
                    supports_operation_id,
                    wire: wire.clone(),
                });
            }
        }
        Ok(Self { tools })
    }

    pub fn set_cancel(&mut self, cancel: Option<Arc<AtomicBool>>) {
        for tool in &self.tools {
            tool.wire.set_cancel(cancel.clone());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

fn handshake(wire: &dyn Wire, name: &str) -> Result<(), String> {
    let mut last = String::new();
    for ver in ["2025-03-26", "2024-11-05"] {
        match wire.rpc(
            "initialize",
            json!({
                "protocolVersion": ver,
                "capabilities": {},
                "clientInfo": { "name": "rung-agent", "version": env!("CARGO_PKG_VERSION") }
            }),
            false,
        ) {
            Ok(_) => {
                wire.rpc("notifications/initialized", json!({}), true)
                    .map_err(|e| format!("mcp {name} initialized: {e}"))?;
                return Ok(());
            }
            Err(e) => last = e,
        }
    }
    Err(format!("mcp {name} initialize: {last}"))
}

impl Toolset for McpRoster {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|t| ToolDefinition::new(&t.name, &t.description, t.input_schema.clone()))
            .collect()
    }

    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        let tool = self.tools.iter().find(|t| t.name == name).ok_or_else(|| {
            let err = McpToolError {
                layer: "mcp".into(),
                method: "tools/call".into(),
                tool: name.to_string(),
                request_id: None,
                operation_id: None,
                retryable: false,
                outcome: "unknown".into(),
                corrective_action: format!("unknown tool: {name}"),
                cause_chain: vec![format!("unknown tool: {name}")],
            };
            let err_str = err.to_json_string();
            eprintln!("[mcp] error: {}", redact(&err_str));
            err_str
        })?;

        // 1. Validate input.is_object BEFORE MCP network
        if !input.is_object() {
            let err = McpToolError {
                layer: "mcp".into(),
                method: "tools/call".into(),
                tool: name.to_string(),
                request_id: None,
                operation_id: None,
                retryable: false,
                outcome: "unknown".into(),
                corrective_action: "params.arguments expected object".into(),
                cause_chain: vec!["params.arguments expected object structural error".into()],
            };
            let err_str = err.to_json_string();
            eprintln!("[mcp] error: {}", redact(&err_str));
            return Err(err_str);
        }

        // 2. Prepare arguments and assign UUID operation_id if supported and missing
        let mut args = input.clone();
        let op_id = if tool.supports_operation_id {
            let existing = args
                .get("operation_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string());
            match existing {
                Some(id) => Some(id),
                None => {
                    let new_uuid = Uuid::new_v4().to_string();
                    args["operation_id"] = json!(new_uuid);
                    Some(new_uuid)
                }
            }
        } else {
            args.get("operation_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string())
        };

        // 3. Retryable ONLY when tool declares idempotency AND actual op id is included
        let can_retry = tool.idempotent_hint && tool.supports_operation_id && op_id.is_some();

        // 4. Dispatch call to wire
        let result = tool
            .wire
            .call_tool(name, &tool.remote, &args, op_id.as_deref(), can_retry)?;

        Ok(content_text(&result))
    }
}

fn content_text(result: &Value) -> String {
    if let Some(arr) = result.get("content").and_then(|c| c.as_array()) {
        let mut parts = Vec::new();
        for item in arr {
            if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                parts.push(t.to_string());
            }
        }
        if !parts.is_empty() {
            return parts.join("\n");
        }
    }
    result.to_string()
}

/// Layer MCP tools over an existing roster.
pub struct WithMcp {
    pub inner: Arc<dyn Toolset>,
    pub mcp: Arc<McpRoster>,
}

impl std::fmt::Debug for WithMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WithMcp").finish()
    }
}

impl Toolset for WithMcp {
    fn definitions(&self) -> Vec<ToolDefinition> {
        let mut d = self.inner.definitions();
        d.extend(self.mcp.definitions());
        d
    }

    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        if self.mcp.tools.iter().any(|t| t.name == name) {
            self.mcp.execute(name, input)
        } else {
            self.inner.execute(name, input)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn parse_http_spec() {
        let s = McpSpec::parse_http("mcp-server=http://mcp-server:8000/mcp").unwrap();
        match s {
            McpSpec::Http { name, url, .. } => {
                assert_eq!(name, "mcp-server");
                assert_eq!(url, "http://mcp-server:8000/mcp");
            }
            _ => panic!("http"),
        }
        assert!(McpSpec::parse_http("nocolon").is_err());
    }

    #[test]
    fn parse_sse_and_json_bodies() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
        assert_eq!(parse_rpc_body(json).unwrap()["result"]["ok"], true);
        let sse = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"n\":1}}\n\n";
        assert_eq!(parse_rpc_body(sse).unwrap()["result"]["n"], 1);
    }

    #[test]
    fn content_text_joins() {
        let v = json!({"content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]});
        assert_eq!(content_text(&v), "a\nb");
    }

    #[test]
    fn parse_rpc_body_preserves_full_diagnostic_on_error() {
        let non_json = "<html><head><title>502 Bad Gateway</title></head><body><h1>Bad Gateway</h1>"
            .to_string() + &"extra diagnostic details ".repeat(20);
        let err = parse_rpc_body(&non_json).unwrap_err();
        assert!(err.starts_with("mcp: no json in body ("));
        assert!(!err.ends_with('…'));
        assert!(err.len() > 200);
    }

    #[test]
    fn test_redact_unicode_safe_dynamic_credentials_and_non_ascii() {
        register_secret("my_super_secret_dynamic_token_123");
        let raw = "こんにちは 🌍 Bearer token_secret_12345678, x-api-key: secret_api_key_xyz, mcp-session-id: session_secret_9876, dynamic: my_super_secret_dynamic_token_123, url: http://admin:super_secret_pw@127.0.0.1:8090/mcp";
        let redacted = redact(raw);
        assert!(!redacted.contains("token_secret_12345678"));
        assert!(!redacted.contains("secret_api_key_xyz"));
        assert!(!redacted.contains("session_secret_9876"));
        assert!(!redacted.contains("my_super_secret_dynamic_token_123"));
        assert!(!redacted.contains("super_secret_pw"));
        assert!(redacted.contains("こんにちは 🌍"));
        assert!(redacted.contains("Bearer [REDACTED]"));
        assert!(redacted.contains("x-api-key: [REDACTED]"));
        assert!(redacted.contains("mcp-session-id: [REDACTED]"));
        assert!(redacted.contains("dynamic: [REDACTED]"));
        assert!(redacted.contains("http://admin:[REDACTED]@127.0.0.1:8090/mcp"));
    }

    enum MockAction {
        Respond(u16, String),
        CloseConnection,
        SleepAndRespond(Duration, u16, String),
    }

    struct MockServer {
        port: u16,
        shutdown: Arc<AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl MockServer {
        fn start<F>(handler: F) -> Self
        where
            F: Fn(&str, &str, &str) -> MockAction + Send + Sync + 'static,
        {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            listener.set_nonblocking(true).unwrap();
            let shutdown = Arc::new(AtomicBool::new(false));
            let s_clone = shutdown.clone();
            let handler = Arc::new(handler);

            let handle = std::thread::spawn(move || {
                while !s_clone.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let _ = stream.set_nonblocking(false);
                            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                            let mut buf = Vec::new();
                            let mut temp = [0u8; 1024];
                            let mut header_end = None;
                            let mut content_len = 0;

                            loop {
                                match stream.read(&mut temp) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        buf.extend_from_slice(&temp[..n]);
                                        if let Some(pos) =
                                            buf.windows(4).position(|w| w == b"\r\n\r\n")
                                        {
                                            header_end = Some(pos + 4);
                                            let header_str = String::from_utf8_lossy(&buf[..pos]);
                                            for line in header_str.lines() {
                                                if let Some((k, v)) = line.split_once(':')
                                                    && k.trim()
                                                        .eq_ignore_ascii_case("content-length")
                                                {
                                                    content_len =
                                                        v.trim().parse::<usize>().unwrap_or(0);
                                                }
                                            }
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }

                            if let Some(h_end) = header_end {
                                while buf.len() < h_end + content_len {
                                    match stream.read(&mut temp) {
                                        Ok(0) => break,
                                        Ok(n) => buf.extend_from_slice(&temp[..n]),
                                        Err(_) => break,
                                    }
                                }

                                let headers_str = String::from_utf8_lossy(&buf[..h_end - 4]);
                                let body_str =
                                    String::from_utf8_lossy(&buf[h_end..h_end + content_len]);
                                let first_line = headers_str.lines().next().unwrap_or("");
                                let mut parts = first_line.split_whitespace();
                                let method = parts.next().unwrap_or("");
                                let path = parts.next().unwrap_or("");

                                match handler(method, path, &body_str) {
                                    MockAction::CloseConnection => {
                                        let _ = stream.shutdown(std::net::Shutdown::Both);
                                        drop(stream);
                                    }
                                    MockAction::Respond(status, body) => {
                                        let resp = format!(
                                            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                                            body.len()
                                        );
                                        let _ = stream.write_all(resp.as_bytes());
                                        let _ = stream.flush();
                                    }
                                    MockAction::SleepAndRespond(dur, status, body) => {
                                        let start = Instant::now();
                                        while start.elapsed() < dur
                                            && !s_clone.load(Ordering::SeqCst)
                                        {
                                            std::thread::sleep(Duration::from_millis(25));
                                        }
                                        if !s_clone.load(Ordering::SeqCst) {
                                            let resp = format!(
                                                "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                                                body.len()
                                            );
                                            let _ = stream.write_all(resp.as_bytes());
                                            let _ = stream.flush();
                                        }
                                    }
                                }
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });

            Self {
                port,
                shutdown,
                handle: Some(handle),
            }
        }

        fn url(&self) -> String {
            format!("http://127.0.0.1:{}/mcp", self.port)
        }
    }

    impl Drop for MockServer {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::SeqCst);
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
        }
    }

    fn handle_mcp_handshake(body_str: &str, tools: Value) -> Option<MockAction> {
        let val: Value = serde_json::from_str(body_str).ok()?;
        let method = val.get("method").and_then(|m| m.as_str())?;
        let id = val.get("id");
        match method {
            "initialize" => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {},
                        "serverInfo": {"name": "mock", "version": "1.0"}
                    }
                });
                Some(MockAction::Respond(200, resp.to_string()))
            }
            "notifications/initialized" => {
                let resp = json!({"jsonrpc": "2.0", "result": null});
                Some(MockAction::Respond(200, resp.to_string()))
            }
            "tools/list" => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": tools
                    }
                });
                Some(MockAction::Respond(200, resp.to_string()))
            }
            _ => None,
        }
    }

    #[test]
    fn test_response_lost_after_synthetic_committed_op_ledger_resend_same_op() {
        let apply_count = Arc::new(AtomicUsize::new(0));
        let ac_clone = apply_count.clone();
        let seen_ops = Arc::new(Mutex::new(HashSet::new()));
        let so_clone = seen_ops.clone();
        let rpc_ids = Arc::new(Mutex::new(Vec::new()));
        let rpc_clone = rpc_ids.clone();
        let op_ids = Arc::new(Mutex::new(Vec::new()));
        let op_clone = op_ids.clone();

        let tools = json!([{
            "name": "idempotent_test_tool",
            "description": "test",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": {"type": "string"},
                    "operation_id": {"type": "string"}
                }
            },
            "annotations": {
                "idempotentHint": true
            }
        }]);

        let server = MockServer::start(move |_method, _path, body| {
            if let Some(action) = handle_mcp_handshake(body, tools.clone()) {
                return action;
            }
            let parsed: Value = serde_json::from_str(body).unwrap();
            if parsed.get("method").and_then(|m| m.as_str()) == Some("tools/call") {
                let id = parsed.get("id").and_then(|v| v.as_u64()).unwrap();
                let args = &parsed["params"]["arguments"];
                let op_id = args
                    .get("operation_id")
                    .and_then(|v| v.as_str())
                    .unwrap()
                    .to_string();

                rpc_clone.lock().unwrap().push(id);
                op_clone.lock().unwrap().push(op_id.clone());

                let mut set = so_clone.lock().unwrap();
                if !set.contains(&op_id) {
                    set.insert(op_id);
                    ac_clone.fetch_add(1, Ordering::SeqCst);
                    // Drop socket: response lost after synthetic committed op ledger
                    return MockAction::CloseConnection;
                } else {
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{"type": "text", "text": "integrated successfully"}]
                        }
                    });
                    return MockAction::Respond(200, resp.to_string());
                }
            }
            MockAction::Respond(404, "not found".into())
        });

        let spec = McpSpec::parse_http(&format!("mock={}", server.url())).unwrap();
        let roster = McpRoster::connect(&[spec]).unwrap();

        let res = roster.execute("idempotent_test_tool", &json!({"content": "new thought"}));
        assert_eq!(res.unwrap(), "integrated successfully");

        // Count judge/apply = 1
        assert_eq!(apply_count.load(Ordering::SeqCst), 1);

        // Same JSON-RPC request id
        let rpcs = rpc_ids.lock().unwrap();
        assert_eq!(rpcs.len(), 2);
        assert_eq!(rpcs[0], rpcs[1]);

        // Same operation id
        let ops = op_ids.lock().unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0], ops[1]);
        assert!(Uuid::parse_str(&ops[0]).is_ok());
    }

    #[test]
    fn test_transient_disconnect() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc_clone = call_count.clone();

        let tools = json!([{
            "name": "idempotent_test_tool",
            "description": "integrate thought",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": {"type": "string"},
                    "operation_id": {"type": "string"}
                }
            },
            "annotations": {
                "idempotentHint": true
            }
        }]);

        let server = MockServer::start(move |_method, _path, body| {
            if let Some(action) = handle_mcp_handshake(body, tools.clone()) {
                return action;
            }
            let parsed: Value = serde_json::from_str(body).unwrap();
            if parsed.get("method").and_then(|m| m.as_str()) == Some("tools/call") {
                let count = cc_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    return MockAction::CloseConnection;
                } else {
                    let id = parsed.get("id");
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{"type": "text", "text": "recovered"}]
                        }
                    });
                    return MockAction::Respond(200, resp.to_string());
                }
            }
            MockAction::Respond(404, "not found".into())
        });

        let spec = McpSpec::parse_http(&format!("mock={}", server.url())).unwrap();
        let roster = McpRoster::connect(&[spec]).unwrap();

        let res = roster.execute("idempotent_test_tool", &json!({"content": "idea"}));
        assert_eq!(res.unwrap(), "recovered");
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_slow_response_beyond_old60_optional_timeouts_constructor_test_shortened_defaults_asserts300()
     {
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(300));
        assert_eq!(HttpWire::DEFAULT_TIMEOUT, Duration::from_secs(300));

        let server = MockServer::start(move |_method, _path, body| {
            let tools = json!([]);
            if let Some(action) = handle_mcp_handshake(body, tools) {
                return action;
            }
            MockAction::SleepAndRespond(Duration::from_millis(250), 200, "{}".into())
        });

        let wire =
            HttpWire::new_with_timeout(server.url(), &[], Duration::from_millis(50)).unwrap();
        let res = wire.call_tool("slow_tool", "slow_tool", &json!({}), None, false);
        let err_str = res.unwrap_err();
        let err: McpToolError = serde_json::from_str(&err_str).unwrap();

        assert_eq!(err.layer, "mcp");
        assert_eq!(err.outcome, "unknown");
        assert!(!err.retryable);
        assert!(err.cause_chain[0].starts_with("timeout: "));
    }

    #[test]
    fn test_malformed_nonobject_no_requests() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc_clone = call_count.clone();

        let tools = json!([{
            "name": "idempotent_test_tool",
            "description": "integrate",
            "inputSchema": {"type": "object"}
        }]);

        let server = MockServer::start(move |_method, _path, body| {
            if let Some(action) = handle_mcp_handshake(body, tools.clone()) {
                return action;
            }
            cc_clone.fetch_add(1, Ordering::SeqCst);
            MockAction::Respond(200, "{}".into())
        });

        let spec = McpSpec::parse_http(&format!("mock={}", server.url())).unwrap();
        let roster = McpRoster::connect(&[spec]).unwrap();

        let res = roster.execute("idempotent_test_tool", &json!("not an object"));
        assert!(res.is_err());
        let err_str = res.unwrap_err();
        let err: McpToolError = serde_json::from_str(&err_str).unwrap();

        assert_eq!(err.layer, "mcp");
        assert_eq!(err.method, "tools/call");
        assert_eq!(err.tool, "idempotent_test_tool");
        assert!(!err.retryable);
        assert_eq!(err.outcome, "unknown");
        assert_eq!(err.corrective_action, "params.arguments expected object");
        assert!(err.cause_chain[0].contains("params.arguments expected object structural error"));

        // No network calls made
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_non_idempotent_transport_fails_no_retry() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc_clone = call_count.clone();

        let tools = json!([{
            "name": "non_idempotent_tool",
            "description": "non idempotent",
            "inputSchema": {"type": "object"},
            "annotations": {
                "idempotentHint": false
            }
        }]);

        let server = MockServer::start(move |_method, _path, body| {
            if let Some(action) = handle_mcp_handshake(body, tools.clone()) {
                return action;
            }
            let parsed: Value = serde_json::from_str(body).unwrap();
            if parsed.get("method").and_then(|m| m.as_str()) == Some("tools/call") {
                cc_clone.fetch_add(1, Ordering::SeqCst);
                return MockAction::CloseConnection;
            }
            MockAction::Respond(404, "not found".into())
        });

        let spec = McpSpec::parse_http(&format!("mock={}", server.url())).unwrap();
        let roster = McpRoster::connect(&[spec]).unwrap();

        let res = roster.execute("non_idempotent_tool", &json!({"key": "val"}));
        assert!(res.is_err());
        let err_str = res.unwrap_err();
        let err: McpToolError = serde_json::from_str(&err_str).unwrap();

        assert_eq!(err.layer, "mcp");
        assert!(!err.retryable);
        assert_eq!(err.outcome, "unknown");

        // Exactly 1 call, no retry
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_is_error_full_causes_no_truncation_redacted_auth() {
        let tools = json!([{
            "name": "failing_tool",
            "description": "failing tool",
            "inputSchema": {"type": "object"}
        }]);

        let sensitive_token = "Bearer secret_jwt_token_987654321_abcd";
        let nested_cause = "CRITICAL_INTERNAL_FAILURE: nested fault at line 42 with credential ";
        let deep_cause_long = nested_cause.repeat(20);
        let error_body = json!({
            "status": "error",
            "errors": ["Database connection dropped"],
            "diagnostic": {"code": "DB_TIMEOUT", "details": deep_cause_long},
            "corrective_action": "Check database status; do not retry with new operation_id."
        });
        let full_error_text = format!("{sensitive_token}\n{}", error_body);

        let err_text_clone = full_error_text.clone();
        let server = MockServer::start(move |_method, _path, body| {
            if let Some(action) = handle_mcp_handshake(body, tools.clone()) {
                return action;
            }
            let parsed: Value = serde_json::from_str(body).unwrap();
            let id = parsed.get("id");
            let resp = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{"type": "text", "text": err_text_clone}],
                    "isError": true
                }
            });
            MockAction::Respond(200, resp.to_string())
        });

        let spec = McpSpec::parse_http(&format!("mock={}", server.url())).unwrap();
        let roster = McpRoster::connect(&[spec]).unwrap();

        let res = roster.execute("failing_tool", &json!({}));
        assert!(res.is_err(), "must honor isError as Err, no fake success");

        let err_str = res.unwrap_err();
        let err: McpToolError = serde_json::from_str(&err_str).unwrap();

        assert_eq!(err.layer, "mcp");
        assert_eq!(err.method, "tools/call");
        assert_eq!(err.tool, "failing_tool");
        assert_eq!(err.outcome, "unknown");
        assert!(!err.retryable);

        // Verify auth is REDACTED
        assert!(!err.cause_chain[0].contains("secret_jwt_token_987654321_abcd"));
        assert!(err.cause_chain[0].contains("Bearer [REDACTED]"));

        // Verify full causes with NO clipping / truncation: entire nested error & diagnostic retained
        assert!(err.cause_chain[0].len() > 1000);
        assert!(err.cause_chain[0].contains(nested_cause));
    }

    #[test]
    fn test_cancel_held_server_returns_quickly_no_retry() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc_clone = call_count.clone();

        let tools = json!([{
            "name": "held_tool",
            "description": "held",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "operation_id": {"type": "string"}
                }
            },
            "annotations": {
                "idempotentHint": true
            }
        }]);

        let server = MockServer::start(move |_method, _path, body| {
            if let Some(action) = handle_mcp_handshake(body, tools.clone()) {
                return action;
            }
            cc_clone.fetch_add(1, Ordering::SeqCst);
            // Server holds connection open for 10 seconds
            MockAction::SleepAndRespond(Duration::from_secs(10), 200, "{}".into())
        });

        let spec = McpSpec::parse_http(&format!("mock={}", server.url())).unwrap();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let roster = McpRoster::connect_with_cancel(&[spec], Some(cancel_flag.clone())).unwrap();

        // Spawn a background thread to cancel after 50ms
        let flag_clone = cancel_flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            flag_clone.store(true, Ordering::SeqCst);
        });

        let start = Instant::now();
        let res = roster.execute("held_tool", &json!({}));
        let elapsed = start.elapsed();

        assert!(res.is_err());
        let err_str = res.unwrap_err();
        let err: McpToolError = serde_json::from_str(&err_str).unwrap();

        assert_eq!(err.outcome, "unknown");
        assert!(!err.retryable);
        assert!(err.cause_chain[0].contains("cancelled"));
        // Returned quickly in < 1 second instead of waiting 10s
        assert!(elapsed < Duration::from_millis(900));
        // No retries
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_output_success_validation_rejects_missing_or_null_result() {
        let tools = json!([{
            "name": "malformed_output_tool",
            "description": "malformed output",
            "inputSchema": {"type": "object"}
        }]);

        let server = MockServer::start(move |_method, _path, body| {
            if let Some(action) = handle_mcp_handshake(body, tools.clone()) {
                return action;
            }
            // Returns null result (missing object content)
            let parsed: Value = serde_json::from_str(body).unwrap();
            let id = parsed.get("id");
            let resp = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": null
            });
            MockAction::Respond(200, resp.to_string())
        });

        let spec = McpSpec::parse_http(&format!("mock={}", server.url())).unwrap();
        let roster = McpRoster::connect(&[spec]).unwrap();

        let res = roster.execute("malformed_output_tool", &json!({}));
        assert!(res.is_err(), "must reject null result as malformed");
        let err_str = res.unwrap_err();
        let err: McpToolError = serde_json::from_str(&err_str).unwrap();

        assert_eq!(err.outcome, "unknown");
        assert!(!err.retryable);
        assert!(err.corrective_action.contains("expected result object"));
    }

    #[test]
    #[ignore]
    fn test_slow_response_65s_timeout_90s() {
        let tools = json!([{
            "name": "long_judge_tool",
            "description": "long judge",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "operation_id": {"type": "string"}
                }
            },
            "annotations": {
                "idempotentHint": true
            }
        }]);

        let server = MockServer::start(move |_method, _path, body| {
            if let Some(action) = handle_mcp_handshake(body, tools.clone()) {
                return action;
            }
            let parsed: Value = serde_json::from_str(body).unwrap();
            let id = parsed.get("id");
            // Sleep for 65 seconds
            MockAction::SleepAndRespond(
                Duration::from_secs(65),
                200,
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{"type": "text", "text": "long judge success after 65s"}]
                    }
                })
                .to_string(),
            )
        });

        let wire = HttpWire::new_with_timeout(server.url(), &[], Duration::from_secs(90)).unwrap();
        let start = Instant::now();
        let res = wire.call_tool(
            "long_judge_tool",
            "long_judge_tool",
            &json!({}),
            None,
            false,
        );
        let elapsed = start.elapsed();

        assert!(res.is_ok());
        assert!(elapsed >= Duration::from_secs(65));
        let val = res.unwrap();
        assert_eq!(val["content"][0]["text"], "long judge success after 65s");
    }
}
