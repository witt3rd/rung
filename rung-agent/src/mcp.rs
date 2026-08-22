//! MCP client (stdio + streamable HTTP). Product, not kernel.
//!
//! Discovers remote tools and admits them as a [`Toolset`]. Harbor hello-mcp
//! is streamable HTTP; ACP requires stdio.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use rung_std::llm::ToolDefinition;
use rung_std::tools::Toolset;
use serde_json::{Value, json};

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
}

struct HttpWire {
    url: String,
    client: Client,
    extra: HeaderMap,
    session: Mutex<Option<String>>,
    next_id: AtomicU64,
}

impl HttpWire {
    fn new(url: String, headers: &[(String, String)]) -> Result<Self, String> {
        let mut extra = HeaderMap::new();
        for (k, v) in headers {
            let name =
                HeaderName::from_bytes(k.as_bytes()).map_err(|e| format!("mcp header {k}: {e}"))?;
            let val = HeaderValue::from_str(v).map_err(|e| format!("mcp header {k}: {e}"))?;
            extra.insert(name, val);
        }
        Ok(Self {
            url,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(|e| e.to_string())?,
            extra,
            session: Mutex::new(None),
            next_id: AtomicU64::new(1),
        })
    }
}

impl Wire for HttpWire {
    fn rpc(&self, method: &str, params: Value, notification: bool) -> Result<Value, String> {
        let mut body = json!({"jsonrpc": "2.0", "method": method, "params": params});
        if !notification {
            body["id"] = json!(self.next_id.fetch_add(1, Ordering::Relaxed));
        }
        let mut req = self
            .client
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
            .map_err(|e| format!("mcp http: {e}"))?;
        if let Some(sid) = resp
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            *self.session.lock().expect("mcp session") = Some(sid.to_string());
        }
        let status = resp.status();
        let text = resp.text().map_err(|e| format!("mcp http body: {e}"))?;
        if !status.is_success() {
            return Err(format!("mcp http {status}: {text}"));
        }
        if notification {
            return Ok(Value::Null);
        }
        let parsed = parse_rpc_body(&text)?;
        if let Some(err) = parsed.get("error") {
            return Err(format!("mcp {method}: {err}"));
        }
        Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
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
    fn rpc(&self, method: &str, params: Value, notification: bool) -> Result<Value, String> {
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
            .map_err(|e| format!("mcp stdio json: {e}: {got}"))?;
        if let Some(err) = parsed.get("error") {
            return Err(format!("mcp {method}: {err}"));
        }
        Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
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
    Err(format!("mcp: no json in body ({})", trunc(trimmed, 180)))
}

fn trunc(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

struct RemoteTool {
    name: String,
    remote: String,
    description: String,
    input_schema: Value,
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
        let mut tools = Vec::new();
        let mut seen: HashMap<String, u32> = HashMap::new();
        for spec in specs {
            let wire: Arc<dyn Wire> = match spec {
                McpSpec::Http { url, headers, .. } => {
                    Arc::new(HttpWire::new(url.clone(), headers)?)
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
                tools.push(RemoteTool {
                    name,
                    remote: raw,
                    description,
                    input_schema,
                    wire: wire.clone(),
                });
            }
        }
        Ok(Self { tools })
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
        let tool = self
            .tools
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| format!("unknown tool: {name}"))?;
        let result = tool.wire.rpc(
            "tools/call",
            json!({"name": tool.remote, "arguments": input}),
            false,
        )?;
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
}
