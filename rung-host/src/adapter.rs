//! The real engine: [`TurnEngine`] over `rung-agent-core`'s
//! [`Engine`], against an OpenAI-compatible
//! route (a router such as OpenRouter, or a local server).
//!
//! - **The host's tools are the turn's whole toolset.** The engine is built
//!   with an empty roster; each turn's host toolset (the stable superset and
//!   its call gate) replaces it, so the definitions sent are the pack's.
//! - **The caller owns the thread (L12).** The turn gets the pack's thread
//!   and gives back what it added, verbatim; nothing the host sent is
//!   rewritten. The host shortens only at a rollover. An in-turn elision
//!   after a context overflow (the engine's last resort) is reported, not
//!   hidden.
//! - **Session and breakpoints (L14).** Each turn's calls carry the epoch's
//!   session id (sticky routing) and two explicit cache breakpoints: the end
//!   of the stable layer (the system text) and of the slow layer (the first
//!   message). The reasoning effort is pinned for the agent's life.
//! - **Per-call usage (L11).** Every call is recorded as served: model,
//!   usage, cached and cache-write tokens, cost, latency on the host clock.
//! - **Who refused.** A 429 is the platform's own (an account quota, which
//!   carries `X-RateLimit-*` and no upstream provider) or a provider's
//!   behind the router (the router's provider metadata in the body). Only a
//!   provider's steps the ladder down; see [`crate::governor`].
//!
//! The key is the operator's: [`AdapterConfig::api_key`] is read by the
//! caller from the environment variable its configuration names, and is
//! never written to the record.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rung_agent_core::catalog::{Kind, Scope};
use rung_agent_core::config::{TurnCheckBackend, TurnCheckSettings};
use rung_agent_core::engine::{
    Engine, EngineSpec, Event, EventSink, ProviderClass, TurnCtl, TurnReport,
};
use rung_agent_core::run::{Status, WrapTools};
use rung_std::agent::FailureKind;
use rung_std::llm::{
    CacheBreakpoint, CachePolicy, ChatMessage, HttpFailure, LlmConfig, Protocol, StreamEvent,
    StreamListener,
};
use rung_std::tools::Toolset;
use serde_json::Value;

use crate::clock::{Clock, Millis};
use crate::core::lock;
use crate::engine::{CallRecord, Ended, EngineTurn, HostFailure, Origin, TurnEngine, TurnRequest};

/// How to reach the route and how to call it.
#[derive(Debug, Clone)]
pub struct AdapterConfig {
    /// The route's base URL, e.g. `https://openrouter.ai/api/v1`.
    pub base_url: String,
    /// The key, read by the caller from its configured env var.
    pub api_key: String,
    pub protocol: Protocol,
    /// The reasoning effort, pinned: varying it can spoil a provider's
    /// cached prefix.
    pub reasoning: Option<String>,
    /// 0: no cap.
    pub max_tokens: u32,
    pub timeout_secs: u64,
    pub idle_timeout_secs: Option<u64>,
    /// Send the epoch's session id on every call.
    pub session_ids: bool,
    /// Mark the stable and slow layers' ends as cache breakpoints.
    pub breakpoints: bool,
    /// Model calls a turn may make.
    pub step_cap: u32,
    /// The engine's workspace (the host's sandbox).
    pub workspace: PathBuf,
    /// The route's name in the record (`llm.call.provider`): the stream
    /// does not name the provider behind a router.
    pub route: String,
}

impl AdapterConfig {
    /// The defaults for an OpenAI-compatible router: session ids and
    /// breakpoints on, reasoning pinned at `medium`, six calls a turn.
    pub fn new(base_url: &str, api_key: &str, workspace: &Path) -> Self {
        Self {
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            protocol: Protocol::OpenAiChat,
            reasoning: Some("medium".into()),
            max_tokens: 0,
            timeout_secs: 120,
            idle_timeout_secs: Some(60),
            session_ids: true,
            breakpoints: true,
            step_cap: 6,
            workspace: workspace.to_path_buf(),
            route: route_of(base_url),
        }
    }
}

/// The host part of a URL (`openrouter.ai`), or the URL.
fn route_of(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    rest.split(['/', ':']).next().unwrap_or(rest).to_string()
}

/// The engine adapter.
pub struct AgentEngine {
    engine: Engine,
    cfg: AdapterConfig,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for AgentEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentEngine")
            .field("route", &self.cfg.route)
            .field("engine", &self.engine)
            .finish()
    }
}

impl AgentEngine {
    pub fn new(cfg: AdapterConfig, clock: Arc<dyn Clock>) -> Result<Self, String> {
        let llm = LlmConfig {
            base_url: cfg.base_url.clone(),
            api_key: cfg.api_key.clone(),
            // Each turn names its model (the ladder's rung).
            model: String::new(),
            timeout_secs: cfg.timeout_secs,
            idle_timeout_secs: cfg.idle_timeout_secs,
            max_tokens: cfg.max_tokens,
            temperature: None,
            top_p: None,
            top_k: None,
            seed: None,
            stop: Vec::new(),
            reasoning_level: cfg.reasoning.clone(),
            structured_outputs: false,
            protocol: cfg.protocol,
            cache: CachePolicy::Auto,
            stream_listener: None,
            session_id: None,
            cache_breakpoints: if cfg.breakpoints {
                vec![CacheBreakpoint::System, CacheBreakpoint::Message(0)]
            } else {
                Vec::new()
            },
        };
        let spec = EngineSpec {
            llm,
            kind: Kind::Explore,
            scope: Scope::parse("none")?,
            max_iterations: Some(cfg.step_cap),
            turn_check: TurnCheckSettings {
                backend: TurnCheckBackend::Off,
                base_url: String::new(),
                model: String::new(),
                api_key_env: String::new(),
                timeout_secs: 1,
            },
            tool_images: false,
            mcp: Vec::new(),
            workspace: cfg.workspace.clone(),
        };
        Ok(Self {
            engine: Engine::new(spec)?,
            cfg,
            clock,
        })
    }
}

/// What the turn's listener saw: one entry per call, and the last refused
/// attempt since the last call that started.
#[derive(Default)]
struct Seen {
    /// (started, ended) on the host clock, per call.
    calls: Vec<(Millis, Millis)>,
    refused: Option<HttpFailure>,
}

struct Listener {
    clock: Arc<dyn Clock>,
    /// When the next call may have been sent: the turn's start, then each
    /// tool's end.
    mark: Arc<Mutex<Millis>>,
    seen: Arc<Mutex<Seen>>,
}

impl StreamListener for Listener {
    fn on_event(&self, event: StreamEvent) {
        let now = self.clock.now();
        let mut s = lock(&self.seen);
        match event {
            StreamEvent::MessageStart { .. } => {
                let from = *lock(&self.mark);
                s.calls.push((from, now));
                s.refused = None;
            }
            StreamEvent::MessageStop | StreamEvent::MessageDelta { .. } => {
                if let Some(last) = s.calls.last_mut() {
                    last.1 = now;
                }
            }
            _ => {}
        }
    }

    fn on_http_failure(&self, failure: &HttpFailure) {
        lock(&self.seen).refused = Some(failure.clone());
    }
}

/// Marks the end of each tool call, so the next call's latency starts there.
struct Marked {
    inner: Arc<dyn Toolset>,
    clock: Arc<dyn Clock>,
    mark: Arc<Mutex<Millis>>,
}

impl std::fmt::Debug for Marked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Marked").finish()
    }
}

impl Toolset for Marked {
    fn definitions(&self) -> Vec<rung_std::llm::ToolDefinition> {
        self.inner.definitions()
    }

    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        let out = self.inner.execute(name, input);
        *lock(&self.mark) = self.clock.now();
        out
    }

    fn execute_output(
        &self,
        name: &str,
        input: &Value,
    ) -> Result<rung_std::tools::ToolOutput, String> {
        let out = self.inner.execute_output(name, input);
        *lock(&self.mark) = self.clock.now();
        out
    }
}

/// The engine's diagnostics go nowhere: the record is the host's account.
struct Silent;

impl EventSink for Silent {
    fn event(&self, _: &Event<'_>) {}
}

/// Who refused a rate-limited call, and when a platform quota resets.
pub fn origin_of(f: Option<&HttpFailure>, now: Millis) -> (Origin, Option<Millis>) {
    let Some(f) = f else {
        // Nothing refused over HTTP: a stream error or a transport failure,
        // the provider's side.
        return (Origin::Provider, None);
    };
    let provider_meta = match serde_json::from_str::<Value>(&f.body) {
        Ok(body) => {
            let meta = &body["error"]["metadata"];
            meta.get("provider_name").is_some() || meta.get("provider_code").is_some()
        }
        Err(_) => f.body.contains("\"provider_name\"") || f.body.contains("\"provider_code\""),
    };
    if provider_meta {
        return (Origin::Provider, None);
    }
    let header = |name: &str| {
        f.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.trim().to_string())
    };
    if let Some(reset) = header("x-ratelimit-reset") {
        return (Origin::Platform, reset_at(&reset, now));
    }
    if f.headers.iter().any(|(k, _)| k.starts_with("x-ratelimit-")) {
        return (Origin::Platform, None);
    }
    (Origin::Provider, None)
}

/// `X-RateLimit-Reset`: ms since the epoch, seconds since the epoch, or
/// seconds from now.
fn reset_at(v: &str, now: Millis) -> Option<Millis> {
    let x: f64 = v.parse().ok()?;
    if x >= 1e12 {
        Some(x as Millis)
    } else if x >= 1e9 {
        Some((x * 1000.0) as Millis)
    } else {
        Some(now + (x * 1000.0) as Millis)
    }
}

impl TurnEngine for AgentEngine {
    fn name(&self) -> &str {
        "agent"
    }

    fn turn(&self, req: TurnRequest) -> EngineTurn {
        let start = self.clock.now();
        let mark = Arc::new(Mutex::new(start));
        let seen = Arc::new(Mutex::new(Seen::default()));
        let tools: Arc<dyn Toolset> = Arc::new(Marked {
            inner: req.tools.clone(),
            clock: self.clock.clone(),
            mark: mark.clone(),
        });
        // The host's toolset replaces the engine's (empty) roster.
        let wrap: WrapTools = Arc::new(move |_roster| tools.clone());
        let ctl = TurnCtl {
            cancel: req.ctl.cancel.clone(),
            stream_listener: Some(Arc::new(Listener {
                clock: self.clock.clone(),
                mark,
                seen: seen.clone(),
            })),
            wrap_tools: vec![wrap],
            sink: Arc::new(Silent),
            model: Some(req.model.clone()),
            session_id: self.cfg.session_ids.then(|| req.session.clone()),
            ..TurnCtl::default()
        };
        let given = req.thread.messages.clone();
        let sent = given.len();
        let report = self.engine.turn(req.thread, ctl);
        let seen = std::mem::take(&mut *lock(&seen));
        map_report(
            report,
            &given,
            sent,
            seen,
            &self.cfg.route,
            self.clock.now(),
        )
    }
}

fn map_report(
    report: TurnReport,
    given: &[ChatMessage],
    sent: usize,
    seen: Seen,
    route: &str,
    now: Millis,
) -> EngineTurn {
    let TurnReport {
        outcome,
        failure,
        calls,
    } = report;
    let (transcript, final_text, mut ended, in_turn_elided) = match &outcome {
        Ok(d) => {
            let ended = match d.status {
                Status::Cancelled => Ended::Cancelled,
                Status::Truncated => Ended::Bounded,
                _ => Ended::Completed,
            };
            (
                &d.result.transcript,
                Some(d.result.final_response.clone()),
                ended,
                d.elided,
            )
        }
        Err(f) => {
            let ended = match f.kind {
                FailureKind::Interrupted => Ended::Cancelled,
                FailureKind::MaxIterations
                | FailureKind::BudgetExhausted
                | FailureKind::DoomLoop => Ended::Bounded,
                _ => Ended::Completed,
            };
            (&f.transcript, None, ended, 0)
        }
    };
    // What the turn added. The given part comes back as it was sent unless
    // an overflow elided it inside the turn; the host keeps its own copy.
    let rewritten = transcript.len() < sent || transcript[..sent] != *given;
    let messages: Vec<ChatMessage> = transcript.iter().skip(sent).cloned().collect();
    let failure = match (&outcome, failure) {
        (Err(_), Some(f)) => {
            ended = Ended::Failed;
            let (origin, reset_at) = match f.class {
                ProviderClass::RateLimit | ProviderClass::Quota => {
                    origin_of(seen.refused.as_ref(), now)
                }
                _ => (Origin::Provider, None),
            };
            Some(HostFailure {
                failure: f,
                origin,
                reset_at,
            })
        }
        _ => None,
    };
    let calls = calls
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let mut c = c;
            if let Some(u) = c.usage.as_mut() {
                let write = u
                    .provider
                    .as_ref()
                    .and_then(|p| p["prompt_tokens_details"]["cache_write_tokens"].as_u64())
                    .unwrap_or(0) as u32;
                u.cache_creation_input_tokens = write;
                u.non_cached_input_tokens = u
                    .input_tokens
                    .saturating_sub(u.cache_read_input_tokens)
                    .saturating_sub(write);
            }
            let latency_ms = seen.calls.get(i).map(|(a, b)| b - a).unwrap_or(0);
            CallRecord {
                usage: c,
                provider: route.to_string(),
                latency_ms,
            }
        })
        .collect();
    EngineTurn {
        messages,
        final_text: final_text.filter(|t| !t.is_empty()),
        calls,
        failure,
        ended,
        rewritten: rewritten || in_turn_elided > 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(headers: &[(&str, &str)], body: &str) -> HttpFailure {
        HttpFailure {
            status: 429,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: body.into(),
        }
    }

    #[test]
    fn a_relayed_provider_429_is_the_providers() {
        let x = f(
            &[("retry-after", "3")],
            r#"{"error":{"code":429,"metadata":{"provider_name":"Up","raw":"busy"}}}"#,
        );
        assert_eq!(origin_of(Some(&x), 0), (Origin::Provider, None));
        // Provider metadata wins over rate-limit headers.
        let y = f(
            &[("x-ratelimit-reset", "1790990000000")],
            r#"{"error":{"metadata":{"provider_name":"Up"}}}"#,
        );
        assert_eq!(origin_of(Some(&y), 0).0, Origin::Provider);
    }

    #[test]
    fn the_routers_own_429_is_the_platforms_with_its_reset() {
        let x = f(&[("x-ratelimit-reset", "1790990000000")], "{}");
        assert_eq!(
            origin_of(Some(&x), 0),
            (Origin::Platform, Some(1_790_990_000_000))
        );
        let s = f(&[("x-ratelimit-reset", "1790990000")], "{}");
        assert_eq!(origin_of(Some(&s), 0).1, Some(1_790_990_000_000));
        let rel = f(&[("x-ratelimit-reset", "30")], "{}");
        assert_eq!(origin_of(Some(&rel), 1_000).1, Some(31_000));
        let none = f(&[("x-ratelimit-remaining", "0")], "{}");
        assert_eq!(origin_of(Some(&none), 0), (Origin::Platform, None));
    }

    #[test]
    fn a_truncated_provider_body_is_still_the_providers() {
        let x = f(
            &[("x-ratelimit-remaining", "0")],
            r#"{"error":{"code":429,"metadata":{"provider_name":"Up","raw":"aaaa"#,
        );
        assert_eq!(origin_of(Some(&x), 0), (Origin::Provider, None));
    }

    #[test]
    fn no_refusal_seen_is_the_providers() {
        assert_eq!(origin_of(None, 0), (Origin::Provider, None));
        assert_eq!(origin_of(Some(&f(&[], "not json")), 0).0, Origin::Provider);
    }

    #[test]
    fn the_route_is_the_urls_host() {
        assert_eq!(route_of("https://openrouter.ai/api/v1"), "openrouter.ai");
        assert_eq!(route_of("http://127.0.0.1:9/v1"), "127.0.0.1");
    }
}
