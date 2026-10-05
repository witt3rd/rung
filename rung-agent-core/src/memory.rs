//! Memory for a turn: recall before the loop, tools during it, retain after.
//!
//! Who owns memory is one setting ([`MemoryAuthority`]): `--memory`, else
//! `RUNG_MEMORY`, else `memory.provider` in `config.yaml`, else `off`.
//!
//! - `off`: nothing here runs and nothing is reported. A turn is what it was
//!   before memory existed.
//! - `external`: the caller owns memory. No store is opened, nothing is
//!   recalled or retained, and no memory tools are added: the agent reaches
//!   memory only through the MCP tools the caller supplies. The outcome says
//!   `{"provider": "external"}` and nothing else.
//! - a provider (`baseline`, `mcp:<url or command>`, or any name registered in
//!   [`registry`]): rung recalls before the turn, admits the provider's tools,
//!   and retains after a turn that holds a [`Completion`].
//!
//! The default scope key is `rung-scope:<hex>`, a hash of the git `origin`
//! URL (else the canonical repo root path), so it reveals no path and a
//! worktree shares memory with its main checkout. A configured `memory.scope`
//! is passed verbatim.
//!
//! The provider sees an opaque scope key and bounded, redacted content, and
//! owns who may see what. rung holds it to rung's caps ([`MAX_RECORDS`],
//! [`MAX_CHARS`]) and to the provider's own declared budget, and times out its
//! calls. A provider that fails never fails a turn: each hook ends in a typed
//! outcome reported in `_meta.rung.memory` and `Outcome.memory`.
//!
//! The recalled block is put after the current user message as quoted
//! data. It is never system text. The session line keeps it beside the
//! user's text (`Line::recalled`), so a later turn replays it. Retain takes
//! a [`Turnover`], and a `Turnover` is built only from a [`Completion`]: a
//! turn that was unverified, unchecked, truncated, cancelled or failed is
//! never retained.
//!
//! The MCP provider contract (`rung-memory/1`) is in `docs/rung-memory.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use rung_memory::{
    Body, Budget, Capability, Charged, Cue, Kept, MemoryAuthority, MemoryProvider, Miss,
    Observation, ProviderSettings, Reach, RecallReport, Recalled, Record, RecordId, Registry,
    RetainReport, Scope, ToolContext, Why,
};
use rung_std::agent::Thread;
use rung_std::llm::{MessageContent, MessageContentBlock, ToolDefinition};
use rung_std::tools::{TEXT_ONLY_CALLER, ToolOutput, Toolset};
use serde::Serialize;
use serde_json::{Value, json};

use crate::config::MemorySettings;
use crate::mcp::{McpRoster, McpSpec, redact};
use crate::turn_check::Completion;

/// The marker an MCP memory provider declares in its `initialize` result,
/// under `capabilities.experimental`.
pub const MARKER: &str = "rung-memory/1";
/// The reserved hook tool rung calls before a turn. Never shown to the agent.
pub const RECALL_TOOL: &str = "rung_memory_recall";
/// The reserved hook tool rung calls after a verified turn. Never shown to
/// the agent.
pub const RETAIN_TOOL: &str = "rung_memory_retain";

/// rung's cap on recalled records, over any provider's declared budget.
pub const MAX_RECORDS: usize = 5;
/// rung's cap on recalled text, in chars.
pub const MAX_CHARS: usize = 4_000;
/// The prompt as sent to a provider, in chars.
pub const PROMPT_CHARS: usize = 2_000;
/// Recent context sent with a recall: this many earlier answers…
pub const CONTEXT_ITEMS: usize = 3;
/// …of at most this many chars each.
pub const CONTEXT_CHARS: usize = 500;
/// Each side of a turn offered to retain, in chars.
pub const TURN_CHARS: usize = 2_000;

/// The providers this build knows: `baseline` and `mcp`.
pub fn registry() -> Registry {
    let mut r = Registry::builtin();
    r.register("mcp", mcp_factory)
        .expect("`mcp` is a valid provider name");
    r
}

// ─── Report ──────────────────────────────────────────────────────────────────

/// Memory's part of an outcome: `_meta.rung.memory` and `Outcome.memory`.
/// Absent while memory is off.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryReport {
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recall: Option<RecallReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain: Option<RetainReport>,
}

// ─── Turnover ────────────────────────────────────────────────────────────────

/// A finished turn offered to retain. Built only from a [`Completion`], so
/// only a turn reported `completed` can become memory.
#[derive(Debug)]
pub struct Turnover {
    observation: Observation,
}

impl Turnover {
    /// The turn `request` → `answer`, the assistant line at `line` of
    /// `session`. Both sides are redacted and bounded.
    pub fn of(done: &Completion, request: &str, answer: &str, session: &str, line: usize) -> Self {
        let checked = if done.was_checked() { "yes" } else { "no" };
        Self {
            observation: Observation {
                body: Body::Turn {
                    user: bound(request, TURN_CHARS),
                    assistant: bound(answer, TURN_CHARS),
                },
                attrs: BTreeMap::from([
                    ("session".into(), session.into()),
                    ("line".into(), line.to_string()),
                    ("source".into(), "turn".into()),
                    ("checked".into(), checked.into()),
                ]),
            },
        }
    }
}

/// Redact, then keep the head and tail of a text over `max` chars.
pub fn bound(text: &str, max: usize) -> String {
    let text = redact(text.trim());
    let n = text.chars().count();
    if n <= max {
        return text;
    }
    let head = max * 3 / 5;
    let tail = max - head;
    let h: String = text.chars().take(head).collect();
    let t: String = text.chars().skip(n - tail).collect();
    format!("{h} […{} chars…] {t}", n - max)
}

// ─── Hooks ───────────────────────────────────────────────────────────────────

/// What the memory setting resolved to for one run.
#[derive(Debug)]
pub enum Hooks {
    Off,
    External,
    On {
        provider: Arc<dyn MemoryProvider>,
        scope: Scope,
    },
}

impl Hooks {
    /// Resolve the setting for a run whose session cwd is `origin`. An
    /// unknown setting is an error; a provider that cannot be reached is not
    /// (its hooks report it unavailable).
    pub fn load(flag: Option<&MemoryAuthority>, origin: &Path) -> Result<Self, String> {
        let settings = crate::config::load_memory(flag)?;
        Self::from_settings(&settings, origin, &registry())
    }

    pub fn from_settings(
        s: &MemorySettings,
        origin: &Path,
        registry: &Registry,
    ) -> Result<Self, String> {
        match &s.authority {
            MemoryAuthority::Off => Ok(Hooks::Off),
            MemoryAuthority::External => Ok(Hooks::External),
            MemoryAuthority::Provider { name, arg } => {
                if !registry.contains(name) {
                    return Err(format!(
                        "memory provider '{name}' is not registered (off | external | {})",
                        registry.names().join(" | ")
                    ));
                }
                let root = repo_root(origin);
                let scope = Scope::new(s.scope.clone().unwrap_or_else(|| default_scope(&root)));
                let dir = s.dir.clone().unwrap_or_else(|| match s.scope {
                    None => root.join(".rung").join("memory"),
                    Some(_) => rung_home().join("memory"),
                });
                let settings = ProviderSettings {
                    dir,
                    arg: arg.clone(),
                    timeout: Duration::from_secs(s.timeout_secs.max(1)),
                    token: s.token.clone(),
                };
                let provider = match registry.build(name, &settings) {
                    Ok(p) => p,
                    Err(why) => {
                        rung_std::events::emit(
                            "rung-agent",
                            "memory.unavailable",
                            &format!("[rung-agent] memory: provider {name} is unavailable ({why})"),
                        );
                        Arc::new(Down {
                            name: name.clone(),
                            why,
                        })
                    }
                };
                Ok(Hooks::On {
                    provider: Arc::new(Capped(provider)),
                    scope,
                })
            }
        }
    }

    /// A report naming the provider, or `None` while memory is off.
    pub fn report(&self) -> Option<MemoryReport> {
        let provider = match self {
            Hooks::Off => return None,
            Hooks::External => "external".to_string(),
            Hooks::On { provider, .. } => provider.name().to_string(),
        };
        Some(MemoryReport {
            provider,
            recall: None,
            retain: None,
        })
    }

    /// The provider's agent tools, for a run of `session`.
    pub fn toolset(&self, session: &str) -> Option<Arc<dyn Toolset>> {
        match self {
            Hooks::Off | Hooks::External => None,
            Hooks::On { provider, scope } => {
                if !provider.capability().tools {
                    return None;
                }
                let ctx = ToolContext {
                    scope: scope.clone(),
                    attrs: BTreeMap::from([("session".into(), session.into())]),
                };
                provider.clone().toolset(&ctx)
            }
        }
    }

    /// Recall for `prompt`, with `recent` earlier answers as context. The
    /// report, and the block to show when something was found.
    pub fn recall(
        &self,
        prompt: &str,
        recent: &[String],
    ) -> Option<(RecallReport, Option<String>)> {
        let Hooks::On { provider, scope } = self else {
            return None;
        };
        if !provider.capability().recall {
            return None;
        }
        let skip = recent.len().saturating_sub(CONTEXT_ITEMS);
        let cue = Cue {
            prompt: bound(prompt, PROMPT_CHARS),
            context: recent[skip..]
                .iter()
                .map(|t| bound(t, CONTEXT_CHARS))
                .collect(),
        };
        let (report, evidence) = rung_memory::recall_outcome(provider.clone(), scope.clone(), cue);
        if report.status == "unavailable" {
            rung_std::events::emit(
                "rung-agent",
                "memory.recall_unavailable",
                &format!(
                    "[rung-agent] memory: recall unavailable ({})",
                    report.reason.as_deref().unwrap_or("")
                ),
            );
        }
        Some((report, evidence.map(|e| e.render())))
    }

    /// Offer a verified turn to retain.
    pub fn retain(&self, turn: Turnover) -> Option<RetainReport> {
        let Hooks::On { provider, scope } = self else {
            return None;
        };
        if !provider.capability().retain {
            return None;
        }
        let report = rung_memory::retain_now(provider.clone(), scope.clone(), turn.observation);
        if report.status == "unretained" {
            rung_std::events::emit(
                "rung-agent",
                "memory.retain_failed",
                &format!(
                    "[rung-agent] memory: retain failed ({})",
                    report.reason.as_deref().unwrap_or("")
                ),
            );
        }
        Some(report)
    }
}

/// The separator between a user message and the recalled block after it.
const BLOCK_SEP: &str = "\n\n---\n";

/// A user message's text with its recalled `block` shown after it: the
/// bytes [`inject`] sends, and the bytes a later turn replays from the
/// session line's `recalled`, so the cached prefix runs through both.
pub fn shown(text: &str, block: &str) -> String {
    format!("{text}{BLOCK_SEP}{block}")
}

/// Put `block` after the thread's last user message, as its own text. At
/// the tail, the ask's bytes come first; the session line keeps the block
/// beside the ask (`Line::recalled`) and a later turn replays the two as
/// [`shown`], so the provider's cached prefix runs through the block too.
pub fn inject(thread: &mut Thread, block: &str) {
    let Some(last) = thread.messages.last_mut() else {
        return;
    };
    if last.role != "user" {
        return;
    }
    let tail = format!("{BLOCK_SEP}{block}");
    last.content = match std::mem::replace(&mut last.content, MessageContent::Text(String::new())) {
        MessageContent::Text(t) => MessageContent::Text(shown(&t, block)),
        MessageContent::Blocks(mut b) => {
            b.push(MessageContentBlock::Text {
                text: tail,
                cache: None,
            });
            MessageContent::Blocks(b)
        }
    };
}

/// The nearest directory at or above `dir` holding `.git`, else `dir`.
pub fn repo_root(dir: &Path) -> PathBuf {
    let mut at = Some(dir);
    while let Some(d) = at {
        if d.join(".git").exists() {
            return d.to_path_buf();
        }
        at = d.parent();
    }
    dir.to_path_buf()
}

/// `rung-scope:<hex>`: a SHA-256 prefix of the repository identity, the git
/// `origin` URL when there is one (so a worktree and its main checkout share
/// a scope, and a move keeps it), else the canonical root path. Never the
/// path itself.
pub fn default_scope(root: &Path) -> String {
    use sha2::{Digest, Sha256};
    let identity = origin_url(root).unwrap_or_else(|| {
        root.canonicalize()
            .unwrap_or_else(|_| root.to_path_buf())
            .to_string_lossy()
            .into_owned()
    });
    let digest = Sha256::digest(identity.as_bytes());
    let hex: String = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();
    format!("rung-scope:{hex}")
}

fn origin_url(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let url = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!url.is_empty()).then_some(url)
}

fn rung_home() -> PathBuf {
    match std::env::var("RUNG_HOME") {
        Ok(h) if !h.trim().is_empty() => PathBuf::from(h),
        _ => PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".rung"),
    }
}

/// Layer a provider's tools over the run's own. The run's tools come first,
/// and a provider tool never shadows one of them.
#[derive(Debug)]
pub struct Layered {
    pub inner: Arc<dyn Toolset>,
    pub outer: Arc<dyn Toolset>,
}

impl Layered {
    fn outer_has(&self, name: &str) -> bool {
        !self.inner.definitions().iter().any(|d| d.name == name)
            && self.outer.definitions().iter().any(|d| d.name == name)
    }
}

impl Toolset for Layered {
    fn definitions(&self) -> Vec<ToolDefinition> {
        let mut d = self.inner.definitions();
        for o in self.outer.definitions() {
            if !d.iter().any(|x| x.name == o.name) {
                d.push(o);
            }
        }
        d
    }
    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        if self.outer_has(name) {
            self.outer.execute(name, input)
        } else {
            self.inner.execute(name, input)
        }
    }
    fn execute_output(&self, name: &str, input: &Value) -> Result<ToolOutput, String> {
        if self.outer_has(name) {
            self.outer.execute_output(name, input)
        } else {
            self.inner.execute_output(name, input)
        }
    }
}

// ─── Provider wrappers ───────────────────────────────────────────────────────

/// A provider held to rung's caps as well as its own budget.
#[derive(Debug)]
struct Capped(Arc<dyn MemoryProvider>);

impl MemoryProvider for Capped {
    fn name(&self) -> &str {
        self.0.name()
    }
    fn capability(&self) -> Capability {
        self.0.capability()
    }
    fn budget(&self) -> Budget {
        let b = self.0.budget();
        Budget {
            max_records: b.max_records.min(MAX_RECORDS),
            max_chars: b.max_chars.min(MAX_CHARS),
            max_cost_usd: b.max_cost_usd,
        }
    }
    fn recall(&self, scope: &Scope, cue: &Cue) -> Result<Charged<Vec<Recalled>>, Miss> {
        self.0.recall(scope, cue)
    }
    fn retain(&self, scope: &Scope, o: &Observation) -> Result<Charged<Kept>, Miss> {
        self.0.retain(scope, o)
    }
    fn toolset(self: Arc<Self>, ctx: &ToolContext) -> Option<Arc<dyn Toolset>> {
        self.0.clone().toolset(ctx)
    }
}

/// A provider that could not be built. Every hook is unavailable, with why.
#[derive(Debug)]
struct Down {
    name: String,
    why: String,
}

impl Down {
    fn miss(&self) -> Miss {
        Miss::new(Why::Unreachable(self.why.clone())).calls(0)
    }
}

impl MemoryProvider for Down {
    fn name(&self) -> &str {
        &self.name
    }
    fn capability(&self) -> Capability {
        Capability {
            recall: true,
            retain: true,
            tools: false,
        }
    }
    fn budget(&self) -> Budget {
        Budget::default()
    }
    fn recall(&self, _: &Scope, _: &Cue) -> Result<Charged<Vec<Recalled>>, Miss> {
        Err(self.miss())
    }
    fn retain(&self, _: &Scope, _: &Observation) -> Result<Charged<Kept>, Miss> {
        Err(self.miss())
    }
}

// ─── The MCP provider ────────────────────────────────────────────────────────

/// A provider that is a separate process the host supplies, over MCP.
/// See `docs/rung-memory.md` for the contract.
#[derive(Debug)]
pub struct McpProvider {
    hooks: Arc<McpRoster>,
    agent: Arc<McpRoster>,
    capability: Capability,
    budget: Budget,
    timeout: Duration,
    /// Set when a call timed out: the server was stopped, and every later
    /// call is unavailable at once.
    dead: Arc<AtomicBool>,
    /// The bearer, if any, kept only to scrub it from error text.
    secret: Option<String>,
}

/// `text` with every copy of the bearer `secret` removed.
fn scrub_token(text: &str, secret: Option<&str>) -> String {
    match secret {
        Some(s) if !s.is_empty() => redact(&text.replace(s, "[redacted]")),
        _ => redact(text),
    }
}

/// Parse `mcp:<arg>`: an `http(s)://` URL, or a command and its arguments
/// split on whitespace (no shell).
pub fn mcp_spec(arg: &str) -> Result<McpSpec, String> {
    let arg = arg.trim();
    if arg.starts_with("http://") || arg.starts_with("https://") {
        return Ok(McpSpec::Http {
            name: "memory".into(),
            url: arg.into(),
            headers: Vec::new(),
        });
    }
    let mut words = arg.split_whitespace();
    let command = words
        .next()
        .ok_or("mcp: needs mcp:<url> or mcp:<command>")?;
    Ok(McpSpec::Stdio {
        name: "memory".into(),
        command: PathBuf::from(command),
        args: words.map(str::to_string).collect(),
        env: Vec::new(),
    })
}

/// Run `f` on its own thread for at most `timeout`. `None` when it ran over.
fn within<T: Send + 'static>(
    timeout: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(timeout).ok()
}

fn mcp_factory(s: &ProviderSettings) -> Result<Arc<dyn MemoryProvider>, String> {
    let spec = mcp_spec(s.arg.as_deref().unwrap_or(""))?;
    let spec = match (spec, &s.token) {
        (McpSpec::Http { name, url, .. }, Some(t)) => McpSpec::Http {
            name,
            url,
            headers: vec![("Authorization".into(), format!("Bearer {}", t.expose()))],
        },
        (spec, _) => spec,
    };
    let secret = s.token.as_ref().map(|t| t.expose().to_string());
    let scrub = |e: String| scrub_token(&e, secret.as_deref());
    let mut p = McpProvider::connect(&spec, s.timeout).map_err(scrub)?;
    p.secret = secret;
    Ok(Arc::new(p))
}

impl McpProvider {
    /// Connect, read the marker, and keep the two hook tools apart from the
    /// tools the agent will see.
    pub fn connect(spec: &McpSpec, timeout: Duration) -> Result<Self, String> {
        let spec_t = spec.clone();
        let (mut roster, init) = within(timeout, move || McpRoster::connect_one(&spec_t, timeout))
            .ok_or_else(|| {
                format!("mcp: no answer to initialize within {}s", timeout.as_secs())
            })??;
        let Some(marker) = init
            .pointer("/capabilities/experimental")
            .and_then(|e| e.get(MARKER))
            .filter(|m| m.is_object())
        else {
            roster.abort();
            return Err(format!(
                "mcp: the server does not declare {MARKER} in capabilities.experimental"
            ));
        };
        let declared = |k: &str| marker.get(k).and_then(Value::as_bool).unwrap_or(false);
        let hooks = roster.split_off(&[RECALL_TOOL, RETAIN_TOOL]);
        let capability = Capability {
            recall: declared("recall") && hooks.has(RECALL_TOOL),
            retain: declared("retain") && hooks.has(RETAIN_TOOL),
            tools: !roster.is_empty(),
        };
        let b = marker.get("budget");
        let num = |k: &str| b.and_then(|b| b.get(k)).and_then(Value::as_u64);
        let budget = Budget {
            max_records: num("max_records").map_or(MAX_RECORDS, |n| n as usize),
            max_chars: num("max_chars").map_or(MAX_CHARS, |n| n as usize),
            max_cost_usd: b
                .and_then(|b| b.get("max_cost_usd"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        };
        Ok(Self {
            hooks: Arc::new(hooks),
            agent: Arc::new(roster),
            capability,
            budget,
            timeout,
            dead: Arc::new(AtomicBool::new(false)),
            secret: None,
        })
    }

    /// Call a hook tool within the timeout. Its structured result, the
    /// calls it reports, and its cost.
    fn hook(&self, tool: &'static str, args: Value) -> Result<(Value, u32, f64), Miss> {
        if self.dead.load(Ordering::SeqCst) {
            return Err(Miss::new(Why::Unreachable("an earlier call timed out".into())).calls(0));
        }
        let hooks = self.hooks.clone();
        let result = match within(self.timeout, move || hooks.call_raw(tool, &args)) {
            Some(Ok(v)) => v,
            Some(Err(e)) => {
                return Err(Miss::new(Why::Unreachable(scrub_token(
                    &e,
                    self.secret.as_deref(),
                ))));
            }
            None => {
                self.dead.store(true, Ordering::SeqCst);
                self.hooks.abort();
                return Err(Miss::new(Why::Unreachable(format!(
                    "{tool}: no answer within {}s",
                    self.timeout.as_secs()
                ))));
            }
        };
        let structured = structured(&result)
            .ok_or_else(|| Miss::new(Why::Malformed(format!("{tool}: no structuredContent"))))?;
        let calls = structured
            .get("calls")
            .and_then(Value::as_u64)
            .map_or(1, |n| n.min(u64::from(u32::MAX)) as u32);
        let cost = structured
            .get("cost_usd")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        Ok((structured, calls, cost))
    }
}

/// `structuredContent`, else the first text item parsed as a JSON object.
fn structured(result: &Value) -> Option<Value> {
    if let Some(s) = result.get("structuredContent").filter(|s| s.is_object()) {
        return Some(s.clone());
    }
    result
        .get("content")?
        .as_array()?
        .iter()
        .find_map(|i| i.get("text").and_then(Value::as_str))
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .filter(Value::is_object)
}

impl MemoryProvider for McpProvider {
    fn name(&self) -> &str {
        "mcp"
    }
    fn capability(&self) -> Capability {
        self.capability
    }
    fn budget(&self) -> Budget {
        self.budget
    }

    fn recall(&self, scope: &Scope, cue: &Cue) -> Result<Charged<Vec<Recalled>>, Miss> {
        let b = self.budget();
        let args = json!({
            "scope": scope.as_str(),
            "prompt": cue.prompt,
            "context": cue.context,
            "budget": {
                "max_records": b.max_records.min(MAX_RECORDS),
                "max_chars": b.max_chars.min(MAX_CHARS),
                "max_cost_usd": b.max_cost_usd,
            },
        });
        let (out, calls, cost) = self.hook(RECALL_TOOL, args)?;
        let malformed = |why: &str| {
            Miss::new(Why::Malformed(format!("{RECALL_TOOL}: {why}")))
                .calls(calls)
                .cost(cost)
        };
        let records = out
            .get("records")
            .and_then(Value::as_array)
            .ok_or_else(|| malformed("no `records` array"))?;
        let mut got = Vec::new();
        for r in records {
            let (Some(id), Some(text)) = (
                r.get("id").and_then(Value::as_str),
                r.get("text").and_then(Value::as_str),
            ) else {
                return Err(malformed("a record without `id` and `text`"));
            };
            let attrs = r
                .get("attrs")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| {
                            let v = v.as_str().map_or_else(|| v.to_string(), str::to_string);
                            (k.clone(), v)
                        })
                        .collect()
                })
                .unwrap_or_default();
            got.push(Recalled {
                record: Record {
                    id: RecordId::new(id),
                    // The provider answers for the scope it was asked about.
                    scope: scope.clone(),
                    text: text.to_string(),
                    observed_at: r
                        .get("observed_at")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    attrs,
                },
                reach: Reach::Hit {
                    score: r.get("score").and_then(Value::as_f64).unwrap_or(0.0),
                },
            });
        }
        Ok(Charged::new(got).calls(calls).cost(cost))
    }

    fn retain(&self, scope: &Scope, o: &Observation) -> Result<Charged<Kept>, Miss> {
        let observation = match &o.body {
            Body::Turn { user, assistant } => {
                json!({"kind": "turn", "user": user, "assistant": assistant, "attrs": o.attrs})
            }
            Body::Note { text } => json!({"kind": "note", "text": text, "attrs": o.attrs}),
        };
        let args = json!({"scope": scope.as_str(), "observation": observation});
        let (out, calls, cost) = self.hook(RETAIN_TOOL, args)?;
        let kept = match out.get("status").and_then(Value::as_str) {
            Some("stored") => Kept::Stored(RecordId::new(
                out.get("id").and_then(Value::as_str).unwrap_or(""),
            )),
            Some("declined") => Kept::Declined(
                out.get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("declined")
                    .to_string(),
            ),
            _ => {
                return Err(Miss::new(Why::Malformed(format!(
                    "{RETAIN_TOOL}: `status` is not stored or declined"
                )))
                .calls(calls)
                .cost(cost));
            }
        };
        Ok(Charged::new(kept).calls(calls).cost(cost))
    }

    fn toolset(self: Arc<Self>, _: &ToolContext) -> Option<Arc<dyn Toolset>> {
        if self.agent.is_empty() {
            return None;
        }
        let hooks = self.hooks.clone();
        Some(Arc::new(Bounded {
            inner: self.agent.clone(),
            timeout: self.timeout,
            dead: self.dead.clone(),
            abort: Box::new(move || hooks.abort()),
        }))
    }
}

/// The provider's agent tools under the per-call memory timeout. A call that
/// runs over stops the server, like a hook that runs over.
struct Bounded {
    inner: Arc<dyn Toolset>,
    timeout: Duration,
    dead: Arc<AtomicBool>,
    abort: Box<dyn Fn() + Send + Sync>,
}

impl std::fmt::Debug for Bounded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bounded")
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl Toolset for Bounded {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.inner.definitions()
    }

    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        self.execute_output(name, input)
            .map(|o| o.into_text(TEXT_ONLY_CALLER))
    }

    fn execute_output(&self, name: &str, input: &Value) -> Result<ToolOutput, String> {
        if self.dead.load(Ordering::SeqCst) {
            return Err("memory: an earlier call timed out".into());
        }
        let (inner, tool, args) = (self.inner.clone(), name.to_string(), input.clone());
        match within(self.timeout, move || inner.execute_output(&tool, &args)) {
            Some(r) => r,
            None => {
                self.dead.store(true, Ordering::SeqCst);
                (self.abort)();
                Err(format!(
                    "memory: {name}: no answer within {}s",
                    self.timeout.as_secs()
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rung_std::llm::ChatMessage;

    #[test]
    fn mcp_spec_reads_a_url_or_a_command() {
        match mcp_spec("http://127.0.0.1:9/mcp").unwrap() {
            McpSpec::Http { url, .. } => assert_eq!(url, "http://127.0.0.1:9/mcp"),
            other => panic!("{other:?}"),
        }
        match mcp_spec(" /bin/provider --flag  x ").unwrap() {
            McpSpec::Stdio { command, args, .. } => {
                assert_eq!(command, PathBuf::from("/bin/provider"));
                assert_eq!(args, ["--flag", "x"]);
            }
            other => panic!("{other:?}"),
        }
        assert!(mcp_spec("  ").is_err());
    }

    #[test]
    fn repo_root_is_the_nearest_git_directory() {
        let base = rung_testkit::TempDir::new("root");
        let deep = base.join("a").join("b");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(repo_root(&deep), deep, "no .git: the dir itself");
        std::fs::create_dir_all(base.join(".git")).unwrap();
        assert_eq!(repo_root(&deep), base.path());
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    #[test]
    fn default_scope_is_opaque_and_shared_by_origin() {
        let base = rung_testkit::TempDir::new("scope");
        let (a, b, c) = (base.join("main"), base.join("wt"), base.join("solo"));
        for d in [&a, &b, &c] {
            std::fs::create_dir_all(d).unwrap();
            git(d, &["init", "-q"]);
        }
        for d in [&a, &b] {
            git(d, &["remote", "add", "origin", "git@example.com:o/r.git"]);
        }
        let (ka, kb, kc) = (default_scope(&a), default_scope(&b), default_scope(&c));
        assert_eq!(ka, kb, "same origin, same scope");
        assert_ne!(ka, kc);
        assert!(ka.starts_with("rung-scope:"));
        for k in [&ka, &kc] {
            assert!(
                !k.contains(base.to_str().unwrap()) && !k.contains("example"),
                "{k}"
            );
        }
    }

    #[test]
    fn configured_scope_is_passed_verbatim() {
        let mut s = MemorySettings {
            authority: MemoryAuthority::Provider {
                name: "baseline".into(),
                arg: None,
            },
            scope: Some("my key".into()),
            dir: Some(rung_testkit::TempDir::new("verb").to_path_buf()),
            timeout_secs: 1,
            token: None,
        };
        let Hooks::On { scope, .. } =
            Hooks::from_settings(&s, Path::new("."), &registry()).unwrap()
        else {
            panic!()
        };
        assert_eq!(scope.as_str(), "my key");
        s.scope = None;
        let Hooks::On { scope, .. } =
            Hooks::from_settings(&s, Path::new("."), &registry()).unwrap()
        else {
            panic!()
        };
        assert!(scope.as_str().starts_with("rung-scope:"));
    }

    #[derive(Debug)]
    struct Hung;
    impl Toolset for Hung {
        fn definitions(&self) -> Vec<ToolDefinition> {
            Vec::new()
        }
        fn execute(&self, _: &str, _: &Value) -> Result<String, String> {
            std::thread::sleep(Duration::from_secs(5));
            Ok("late".into())
        }
    }

    #[test]
    fn a_hung_provider_tool_errors_within_the_timeout() {
        let aborted = Arc::new(AtomicBool::new(false));
        let flag = aborted.clone();
        let t = Bounded {
            inner: Arc::new(Hung),
            timeout: Duration::from_millis(100),
            dead: Arc::new(AtomicBool::new(false)),
            abort: Box::new(move || flag.store(true, Ordering::SeqCst)),
        };
        let started = std::time::Instant::now();
        let e = t.execute_output("x", &json!({})).unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(e.contains("no answer within"), "{e}");
        assert!(aborted.load(Ordering::SeqCst));
        assert!(
            t.execute_output("x", &json!({}))
                .unwrap_err()
                .contains("earlier call")
        );
    }

    #[test]
    fn inject_follows_the_last_user_message() {
        let mut t = Thread {
            system_prompt: "sys".into(),
            messages: vec![ChatMessage::user("first"), ChatMessage::user("ask")],
        };
        inject(&mut t, "## block");
        assert_eq!(t.system_prompt, "sys");
        assert_eq!(t.messages[0].content.as_text(), Some("first"));
        assert_eq!(
            t.messages[1].content.as_text(),
            Some("ask\n\n---\n## block")
        );
    }

    #[test]
    fn bound_redacts_and_keeps_head_and_tail() {
        let long = format!("{}{}", "a".repeat(3_000), "z".repeat(10));
        let b = bound(&long, 100);
        assert!(
            b.starts_with(&"a".repeat(60)) && b.ends_with(&"z".repeat(10)),
            "{b}"
        );
    }

    #[test]
    fn an_unknown_provider_is_a_config_error() {
        let s = MemorySettings {
            authority: MemoryAuthority::provider("nope"),
            scope: None,
            dir: None,
            timeout_secs: 1,
            token: None,
        };
        let e = Hooks::from_settings(&s, Path::new("/tmp"), &registry()).unwrap_err();
        assert!(e.contains("baseline | mcp"), "{e}");
    }

    #[test]
    fn an_unreachable_provider_reports_instead_of_failing() {
        let s = MemorySettings {
            authority: MemoryAuthority::Provider {
                name: "mcp".into(),
                arg: Some("/no/such/memory-provider".into()),
            },
            scope: Some("k".into()),
            dir: None,
            timeout_secs: 1,
            token: None,
        };
        let hooks = Hooks::from_settings(&s, Path::new("/tmp"), &registry()).unwrap();
        let (r, block) = hooks.recall("what is the deploy branch", &[]).unwrap();
        assert_eq!(r.status, "unavailable");
        assert!(block.is_none());
        assert!(hooks.toolset("s").is_none());
    }
    /// A one-thread HTTP MCP memory provider that records the
    /// `Authorization` header of every request and answers 401 without the
    /// expected bearer.
    fn bearer_server(want: &'static str) -> (String, Arc<std::sync::Mutex<Vec<Option<String>>>>) {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/mcp", l.local_addr().unwrap());
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for mut c in l.incoming().flatten() {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let (head, len) = loop {
                    let n = c.read(&mut tmp).unwrap_or(0);
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let h = String::from_utf8_lossy(&buf[..p]).to_string();
                        let len = h
                            .lines()
                            .find_map(|l| {
                                let (k, v) = l.split_once(':')?;
                                k.eq_ignore_ascii_case("content-length")
                                    .then(|| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        buf.drain(..p + 4);
                        break (h, len);
                    }
                    if n == 0 {
                        break (String::new(), 0);
                    }
                };
                while buf.len() < len {
                    let n = c.read(&mut tmp).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                }
                let auth = head.lines().find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("authorization")
                        .then(|| v.trim().to_string())
                });
                log.lock().unwrap().push(auth.clone());
                let body: Value = serde_json::from_slice(&buf).unwrap_or(json!({}));
                let id = body.get("id").cloned();
                let result = match body.get("method").and_then(Value::as_str) {
                    Some("initialize") => json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {"experimental": {MARKER: {"recall": true, "retain": true}}},
                        "serverInfo": {"name": "p", "version": "1"}}),
                    Some("tools/list") => json!({"tools": [
                        {"name": RECALL_TOOL, "description": "r", "inputSchema": {"type": "object"}},
                        {"name": RETAIN_TOOL, "description": "r", "inputSchema": {"type": "object"}}]}),
                    Some("tools/call") => json!({"content": [], "structuredContent": {
                        "records": [{"id": "r1", "text": "one"}, {"id": "r2", "text": "two"}]}}),
                    _ => json!(null),
                };
                let (status, out) = if auth.as_deref() == Some(want) {
                    (
                        "200 OK",
                        json!({"jsonrpc": "2.0", "id": id, "result": result}),
                    )
                } else {
                    ("401 Unauthorized", json!({}))
                };
                let out = out.to_string();
                let _ = write!(
                    c,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
                    out.len()
                );
            }
        });
        (url, seen)
    }

    fn http_settings(url: &str, token: Option<&str>) -> MemorySettings {
        MemorySettings {
            authority: MemoryAuthority::Provider {
                name: "mcp".into(),
                arg: Some(url.into()),
            },
            scope: Some("k".into()),
            dir: None,
            timeout_secs: 5,
            token: token.map(|t| rung_memory::Token::new(t).unwrap()),
        }
    }

    #[test]
    fn the_bearer_rides_every_call_and_recall_lists_injected_ids() {
        let secret = "s3cret-bearer-value";
        let (url, seen) = bearer_server("Bearer s3cret-bearer-value");
        let s = http_settings(&url, Some(secret));
        assert!(!format!("{s:?}").contains(secret));
        let hooks = Hooks::from_settings(&s, Path::new("/tmp"), &registry()).unwrap();
        let (r, block) = hooks.recall("what", &[]).unwrap();
        assert_eq!(r.status, "found");
        assert_eq!(r.injected, ["r1", "r2"]);
        assert!(block.is_some());
        let seen = seen.lock().unwrap();
        assert!(seen.len() >= 4, "{seen:?}");
        assert!(
            seen.iter()
                .all(|a| a.as_deref() == Some("Bearer s3cret-bearer-value")),
            "{seen:?}"
        );
        let wire = serde_json::to_string(&hooks.report().unwrap()).unwrap()
            + &serde_json::to_string(&r).unwrap()
            + &format!("{r:?}{block:?}");
        assert!(!wire.contains(secret), "{wire}");
    }

    #[test]
    fn a_wrong_or_missing_bearer_is_unavailable_and_never_echoes_the_token() {
        let (url, _) = bearer_server("Bearer right-right-right");
        for token in [Some("wrong-wrong-wrong"), None] {
            let s = http_settings(&url, token);
            let hooks = Hooks::from_settings(&s, Path::new("/tmp"), &registry()).unwrap();
            let (r, _) = hooks.recall("what", &[]).unwrap();
            assert_eq!(r.status, "unavailable");
            let text = format!("{r:?}");
            assert!(!text.contains("wrong-wrong-wrong"), "{text}");
        }
    }
}
