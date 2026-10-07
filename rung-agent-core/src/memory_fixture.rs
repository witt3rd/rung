//! A reference memory provider: a stdio MCP server that speaks the
//! `rung-memory/1` contract (`docs/rung-memory.md`), and the conformance
//! check that runs the contract against any provider.
//!
//! ```text
//! rung-agent --memory-fixture [--file PATH] [--sleep-ms N] [--no-marker]
//! rung-agent --memory-check SETTING        # e.g. mcp:http://127.0.0.1:9000/mcp
//! ```
//!
//! The fixture keeps records in memory, or appended to `--file` so they
//! outlive the process (rung starts a stdio provider once per prompt). It
//! matches by shared words, charges $0.0001 per hook call, and offers one
//! agent tool, `memory_lookup`. `--sleep-ms` delays every hook call;
//! `--no-marker` leaves the marker out; `--junk` answers every hook
//! call with text that is not a result. These exist to test rung's side.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rung_memory::{Body, Cue, MemoryAuthority, Observation, Scope};
use serde_json::{Value, json};

use crate::memory::{MARKER, RECALL_TOOL, RETAIN_TOOL};

/// What one hook call costs at the fixture.
pub const FIXTURE_COST_USD: f64 = 0.0001;

#[derive(Debug, Default)]
struct Fixture {
    file: Option<PathBuf>,
    sleep: Duration,
    marker: bool,
    junk: bool,
    records: Vec<Value>,
}

/// Serve the fixture on stdin/stdout until stdin closes.
pub fn serve(argv: &[String]) -> Result<(), String> {
    let mut f = Fixture {
        marker: true,
        ..Fixture::default()
    };
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--file" => f.file = Some(PathBuf::from(it.next().ok_or("--file needs a path")?)),
            "--sleep-ms" => {
                let n: u64 = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--sleep-ms needs a number")?;
                f.sleep = Duration::from_millis(n);
            }
            "--no-marker" => f.marker = false,
            "--junk" => f.junk = true,
            other => return Err(format!("memory fixture: unknown option {other}")),
        }
    }
    if let Some(p) = &f.file {
        f.records = load(p);
    }
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let Ok(msg) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let Some(id) = msg.get("id").cloned() else {
            continue; // a notification
        };
        let reply = match f.handle(&msg) {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32602, "message": e}}),
        };
        writeln!(out, "{reply}").map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn load(p: &Path) -> Vec<Value> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4)
        .map(str::to_lowercase)
        .collect()
}

fn tool_result(structured: Value) -> Value {
    json!({
        "content": [{"type": "text", "text": structured.to_string()}],
        "structuredContent": structured,
    })
}

impl Fixture {
    fn handle(&mut self, msg: &Value) -> Result<Value, String> {
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        match msg.get("method").and_then(Value::as_str).unwrap_or("") {
            "initialize" => {
                let mut caps = json!({"tools": {}});
                if self.marker {
                    caps["experimental"] = json!({MARKER: {
                        "recall": true,
                        "retain": true,
                        "budget": {"max_records": 5, "max_chars": 4000, "max_cost_usd": 0.01}
                    }});
                }
                Ok(json!({
                    "protocolVersion": params.get("protocolVersion").cloned().unwrap_or(json!("2025-03-26")),
                    "capabilities": caps,
                    "serverInfo": {"name": "rung-memory-fixture", "version": env!("CARGO_PKG_VERSION")}
                }))
            }
            "tools/list" => Ok(json!({"tools": [
                {"name": RECALL_TOOL, "description": "rung hook: recall", "inputSchema": {"type": "object"}},
                {"name": RETAIN_TOOL, "description": "rung hook: retain", "inputSchema": {"type": "object"}},
                {"name": "memory_lookup", "description": "Look up kept notes by a word.",
                 "inputSchema": {"type": "object", "properties": {"word": {"type": "string"}}, "required": ["word"]}}
            ]})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                self.call(name, &args)
            }
            other => Err(format!("method not found: {other}")),
        }
    }

    fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        let s = |k: &str| {
            args.get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        if self.junk && (name == RECALL_TOOL || name == RETAIN_TOOL) {
            return Ok(
                json!({"content": [{"type": "text", "text": "<html>502 bad gateway</html>"}]}),
            );
        }
        match name {
            RECALL_TOOL => {
                std::thread::sleep(self.sleep);
                let scope = s("scope");
                let want = words(&s("prompt"));
                let max = args
                    .pointer("/budget/max_records")
                    .and_then(Value::as_u64)
                    .map_or(usize::MAX, |n| n as usize);
                let records: Vec<Value> = self
                    .records
                    .iter()
                    .rev()
                    .filter(|r| r["scope"] == scope.as_str())
                    .filter(|r| {
                        let have = words(r["text"].as_str().unwrap_or(""));
                        want.iter().any(|w| have.contains(w))
                    })
                    .take(max)
                    .map(|r| {
                        json!({"id": r["id"], "text": r["text"], "attrs": r["attrs"], "score": 1.0})
                    })
                    .collect();
                Ok(tool_result(
                    json!({"records": records, "cost_usd": FIXTURE_COST_USD, "calls": 1}),
                ))
            }
            RETAIN_TOOL => {
                std::thread::sleep(self.sleep);
                let o = args.get("observation").cloned().unwrap_or(json!({}));
                let text = match o.get("kind").and_then(Value::as_str) {
                    Some("turn") => format!(
                        "User: {}\nAssistant: {}",
                        o["user"].as_str().unwrap_or(""),
                        o["assistant"].as_str().unwrap_or("")
                    ),
                    _ => o["text"].as_str().unwrap_or("").to_string(),
                };
                if text.trim().is_empty() {
                    return Ok(tool_result(
                        json!({"status": "declined", "reason": "empty", "cost_usd": FIXTURE_COST_USD}),
                    ));
                }
                let id = format!("f{}", self.records.len() + 1);
                let record =
                    json!({"id": id, "scope": s("scope"), "text": text, "attrs": o["attrs"]});
                if let Some(p) = &self.file {
                    let mut f = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(p)
                        .map_err(|e| e.to_string())?;
                    writeln!(f, "{record}").map_err(|e| e.to_string())?;
                }
                self.records.push(record);
                Ok(tool_result(
                    json!({"status": "stored", "id": id, "cost_usd": FIXTURE_COST_USD}),
                ))
            }
            "memory_lookup" => {
                let w = s("word").to_lowercase();
                let hits: Vec<&str> = self
                    .records
                    .iter()
                    .filter_map(|r| r["text"].as_str())
                    .filter(|t| t.to_lowercase().contains(&w))
                    .collect();
                Ok(
                    json!({"content": [{"type": "text", "text": if hits.is_empty() { "nothing".to_string() } else { hits.join("\n") }}]}),
                )
            }
            other => Err(format!("unknown tool {other}")),
        }
    }
}

// ─── Conformance ─────────────────────────────────────────────────────────────

/// One clause of the contract, and whether the provider met it.
#[derive(Debug, Clone, PartialEq)]
pub struct Clause {
    pub name: &'static str,
    pub result: Result<String, String>,
}

/// Run the contract against the provider `setting` names. It writes one
/// probe note into a scope of its own (`rung-memory-check:<uuid>`).
pub fn check(
    setting: &str,
    dir: &Path,
    timeout: Duration,
    token: Option<rung_memory::Token>,
) -> Result<Vec<Clause>, String> {
    let authority = MemoryAuthority::parse(setting)?;
    let MemoryAuthority::Provider { name, arg } = &authority else {
        return Err(format!(
            "'{setting}' is a memory setting, not a provider: nothing to check"
        ));
    };
    let registry = crate::memory::registry();
    let provider = registry.build(
        name,
        &rung_memory::ProviderSettings {
            dir: dir.to_path_buf(),
            arg: arg.clone(),
            timeout,
            retain_timeout: timeout,
            token,
        },
    );
    let mut out = Vec::new();
    let mut clause = |name: &'static str, result: Result<String, String>| {
        out.push(Clause { name, result });
    };
    let provider = match provider {
        Ok(p) => {
            clause("connects and declares the provider", Ok(p.name().into()));
            p
        }
        Err(e) => {
            clause("connects and declares the provider", Err(e));
            return Ok(out);
        }
    };
    let cap = provider.capability();
    clause(
        "declares recall and retain",
        if cap.recall && cap.retain {
            Ok("both declared".into())
        } else {
            Err(format!("recall={} retain={}", cap.recall, cap.retain))
        },
    );
    let probe = uuid::Uuid::new_v4().simple().to_string();
    let scope = Scope::new(format!("rung-memory-check:{probe}"));
    let other = Scope::new(format!("rung-memory-check:{probe}:other"));
    let ctx = rung_memory::ToolContext {
        scope: scope.clone(),
        attrs: BTreeMap::new(),
    };
    let names: Vec<String> = provider
        .clone()
        .toolset(&ctx)
        .map(|t| t.definitions().into_iter().map(|d| d.name).collect())
        .unwrap_or_default();
    clause(
        "keeps the hook tools from the agent",
        if names.iter().any(|n| n == RECALL_TOOL || n == RETAIN_TOOL) {
            Err(format!("agent tools include a hook: {names:?}"))
        } else {
            Ok(format!("agent tools: {names:?}"))
        },
    );
    let text = format!("conformance probe {probe} keeps the marigold token");
    let kept = rung_memory::retain_now(
        provider.clone(),
        scope.clone(),
        Observation {
            body: Body::Note { text: text.clone() },
            attrs: BTreeMap::from([("source".into(), "conformance".into())]),
        },
    );
    clause(
        "retain stores a note",
        match kept.status {
            "stored" => Ok(format!("id {}", kept.id.unwrap_or_default())),
            s => Err(format!("{s}: {}", kept.reason.unwrap_or_default())),
        },
    );
    let budget = provider.budget();
    let (found, evidence) =
        rung_memory::recall_outcome(provider.clone(), scope.clone(), Cue::new(&text));
    clause(
        "recall finds it in its scope",
        match &evidence {
            Some(e) if e.items().iter().any(|r| r.record.text.contains(&probe)) => {
                Ok(format!("{} record(s)", e.items().len()))
            }
            _ => Err(format!(
                "{}: {}",
                found.status,
                found.reason.clone().unwrap_or_default()
            )),
        },
    );
    clause(
        "recall stays within its declared budget",
        if found.records <= budget.max_records && found.trace.cost_usd() <= budget.max_cost_usd {
            Ok(format!(
                "{} record(s), ${} of ${}",
                found.records,
                found.trace.cost_usd(),
                budget.max_cost_usd
            ))
        } else {
            Err(format!("{found:?} against {budget:?}"))
        },
    );
    let (elsewhere, leaked) = rung_memory::recall_outcome(provider, other, Cue::new(&text));
    clause(
        "recall does not cross scopes",
        match leaked {
            Some(e) if e.items().iter().any(|r| r.record.text.contains(&probe)) => {
                Err("the probe came back in another scope".into())
            }
            _ if elsewhere.status == "unavailable" => Err(format!(
                "unavailable: {}",
                elsewhere.reason.unwrap_or_default()
            )),
            _ => Ok(elsewhere.status.into()),
        },
    );
    Ok(out)
}

/// Print `check`'s clauses; true when every one passed.
pub fn report(clauses: &[Clause]) -> bool {
    let mut ok = true;
    for c in clauses {
        match &c.result {
            Ok(d) => println!("pass  {}: {d}", c.name),
            Err(d) => {
                ok = false;
                println!("FAIL  {}: {d}", c.name);
            }
        }
    }
    ok
}
