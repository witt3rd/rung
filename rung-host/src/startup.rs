//! The startup-handoff ladder: from the operator's configuration file to a
//! host running its Presence loop.
//!
//! ```text
//! Configured(Plan) => Listed(Plan) => Recovered(Opening) => { Handed(Handoff) | Refused(Refusal) }
//! ```
//!
//! - **Configured.** [`configure`] reads `rung-host.yaml` (unknown fields
//!   refused), checks it, reads the keys from the env vars it names, and
//!   builds the engine. It is the only way to a [`Plan`]; a refusal touches
//!   no state.
//! - **Listed.** The router's models are listed at start, before the record
//!   is opened ([`crate::ladder`]); the verdicts go to the host, which
//!   records them (`ladder.listed`, `at_start`) at its first boundary,
//!   before its first turn. A failed listing does not stop the start.
//! - **Recovered.** The record is opened and replayed (a restart recovers
//!   here).
//! - **Handed** to the Presence loop, with ACP outward when configured; or
//!   **Refused** when the record cannot be opened.
//!
//! Each stage is a rung: a mid-ladder token has no public constructor, so
//! no stage can be skipped or forged.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rung::ladder;
use serde::Deserialize;
use serde_json::Value;

use crate::adapter::{AdapterConfig, AgentEngine};
use crate::calendar::{Entry, Missed, Origin, When};
use crate::clock::{Clock, RealClock, SECOND};
use crate::core::HostConfig;
use crate::desk::{DecisionDesk, DeskMode, SpendCap};
use crate::engine::TurnEngine;
use crate::governor::Quota;
use crate::inbox::{DirSource, Role, Source};
use crate::ladder::{HttpLister, Lister, OPENROUTER_FREE_LADDER};
use crate::memory::MemoryHost;
use crate::notify::Notifier;
use crate::presence::{Host, HostBuilder, Limits, Recovered as Woken};
use crate::stop::{StopAuthority, Why};

// ─── The file ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct EngineFile {
    /// `agent` (the real adapter) or `mock`.
    kind: String,
    #[serde(default)]
    base_url: Option<String>,
    /// The env var holding the route's key; never the key.
    #[serde(default)]
    api_key_env: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    step_cap: Option<u32>,
    #[serde(default)]
    timeout_s: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuotaFile {
    rpd: u64,
    rpm: u64,
    #[serde(default)]
    reserve: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcpFile {
    /// Serve one local client on stdio as this role.
    #[serde(default)]
    stdio: Option<String>,
    /// Serve Streamable HTTP on this address.
    #[serde(default)]
    http: Option<String>,
    /// role → the env var holding its bearer token.
    #[serde(default)]
    tokens: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedProject {
    id: String,
    title: String,
    why: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CalendarFile {
    id: String,
    /// Seconds after the first start.
    in_s: i64,
    text: String,
    #[serde(default)]
    firm: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeskFile {
    /// `decide`, `shadow` or `rule_only`.
    mode: String,
    /// `jev` (System One through `base_url`); absent with `rule_only`.
    #[serde(default)]
    decider: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    /// The env var holding the decider's key; never the key.
    #[serde(default)]
    api_key_env: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    cap_usd_day: Option<f64>,
    #[serde(default)]
    cap_usd_ask: Option<f64>,
    /// While this file exists the decider is not asked.
    #[serde(default)]
    kill_file: Option<PathBuf>,
}

/// `rung-host.yaml`. Keys are named by env var, never written here.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    state: PathBuf,
    #[serde(default)]
    workspace: Option<PathBuf>,
    #[serde(default)]
    identity: Option<String>,
    #[serde(default)]
    owner_channel: Option<String>,
    engine: EngineFile,
    /// Best first; the ruled free ladder when absent.
    #[serde(default)]
    ladder: Option<Vec<String>>,
    /// List the router's models at start and every six hours (default on
    /// for the real engine).
    #[serde(default)]
    listing: Option<bool>,
    #[serde(default)]
    quota: Option<QuotaFile>,
    #[serde(default)]
    epoch_budget_tokens: Option<usize>,
    #[serde(default)]
    turn_bound_s: Option<i64>,
    #[serde(default)]
    backoff_base_ms: Option<i64>,
    /// Baseline memory under the state directory (default on).
    #[serde(default)]
    memory: Option<bool>,
    #[serde(default)]
    seed_projects: Vec<SeedProject>,
    #[serde(default)]
    acp: Option<AcpFile>,
    /// A `*.msg` directory stimulus source.
    #[serde(default)]
    inbox: Option<PathBuf>,
    /// The stop authority also halts when this file appears.
    #[serde(default)]
    stop_file: Option<PathBuf>,
    /// Stop at the first boundary this many seconds after start.
    #[serde(default)]
    run_for_s: Option<i64>,
    /// Owner calendar entries, seeded on the first start only.
    #[serde(default)]
    calendar: Vec<CalendarFile>,
    /// The decision desk (rule-only when absent).
    #[serde(default)]
    desk: Option<DeskFile>,
}

// ─── The stages' payloads ────────────────────────────────────────────────────

/// How ACP outward is served.
#[derive(Debug, Clone)]
pub enum AcpPlan {
    None,
    Stdio(Role),
    Http {
        addr: String,
        tokens: Vec<(String, Role)>,
    },
}

/// A checked configuration, ready to list and open. Built only by
/// [`configure`].
pub struct Plan {
    state: PathBuf,
    config: HostConfig,
    clock: Arc<dyn Clock>,
    engine: Arc<dyn TurnEngine>,
    lister: Option<Arc<dyn Lister>>,
    listing: Option<Value>,
    memory: bool,
    limits: Limits,
    acp: AcpPlan,
    desk: DecisionDesk,
    inbox: Option<PathBuf>,
    stop_file: Option<PathBuf>,
    calendar: Vec<Entry>,
}

impl std::fmt::Debug for Plan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plan")
            .field("state", &self.state)
            .field("ladder", &self.config.ladder)
            .field("listing", &self.listing.is_some())
            .finish()
    }
}

/// The record opened (or not), with what the handoff needs.
pub struct Opening {
    opened: Result<(Arc<Host>, Woken), String>,
    acp: AcpPlan,
}

/// A host ready to run: built only by the ladder's last step.
pub struct Handoff {
    host: Arc<Host>,
    recovered: Woken,
    acp: AcpPlan,
}

/// Why a start was refused, and at which stage.
#[derive(Debug, Clone)]
pub struct Refusal {
    pub stage: &'static str,
    pub why: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.stage, self.why)
    }
}

fn refuse(why: impl Into<String>) -> Refusal {
    Refusal {
        stage: "configured",
        why: why.into(),
    }
}

fn parse_role(field: &str, s: &str) -> Result<Role, Refusal> {
    match s {
        "owner" => Ok(Role::Owner),
        "peer" => Ok(Role::Peer),
        "observer" => Ok(Role::Observer),
        other => Err(refuse(format!("{field}: unknown role `{other}`"))),
    }
}

fn env(field: &str, name: &str) -> Result<String, Refusal> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| refuse(format!("{field}: the env var {name} is not set")))
}

/// Read and check the operator's file: the only way to a [`Configured`]
/// token. `max_turns` bounds the run (tests and bounded runs).
pub fn configure(path: &Path, max_turns: Option<u64>) -> Result<Configured, Refusal> {
    let text =
        std::fs::read_to_string(path).map_err(|e| refuse(format!("{}: {e}", path.display())))?;
    let f: FileConfig =
        serde_yaml::from_str(&text).map_err(|e| refuse(format!("{}: {e}", path.display())))?;
    let clock: Arc<dyn Clock> = Arc::new(RealClock);
    let workspace = f
        .workspace
        .clone()
        .unwrap_or_else(|| f.state.join("workspace"));
    let mut config = HostConfig::new(workspace.clone());
    config.engine = f.engine.kind.clone();
    if let Some(i) = &f.identity {
        config.identity = i.clone();
    }
    if let Some(o) = &f.owner_channel {
        config.owner_channel = o.clone();
    }
    config.ladder = f.ladder.clone().unwrap_or_else(|| {
        OPENROUTER_FREE_LADDER
            .iter()
            .map(|s| s.to_string())
            .collect()
    });
    if config.ladder.is_empty() {
        return Err(refuse("ladder: no rungs"));
    }
    if let Some(q) = &f.quota {
        config.governor.quota = Some(Quota {
            rpd: q.rpd,
            rpm: q.rpm,
            reserve: q.reserve.unwrap_or(0.25),
        });
    }
    if let Some(b) = f.epoch_budget_tokens {
        config.epoch_budget_tokens = b;
    }
    if let Some(s) = f.turn_bound_s {
        config.turn_bound_ms = s * SECOND;
    }
    if let Some(ms) = f.backoff_base_ms {
        config.governor.backoff_base_ms = ms;
    }
    config.seed_projects = f
        .seed_projects
        .iter()
        .map(|p| (p.id.clone(), p.title.clone(), p.why.clone()))
        .collect();
    let (engine, lister): (Arc<dyn TurnEngine>, Option<Arc<dyn Lister>>) =
        match f.engine.kind.as_str() {
            "agent" => {
                let base = f
                    .engine
                    .base_url
                    .clone()
                    .ok_or_else(|| refuse("engine.base_url: required for the agent engine"))?;
                let key_env =
                    f.engine.api_key_env.clone().ok_or_else(|| {
                        refuse("engine.api_key_env: required for the agent engine")
                    })?;
                let key = env("engine.api_key_env", &key_env)?;
                let mut ac = AdapterConfig::new(&base, &key, &workspace);
                if f.engine.reasoning.is_some() {
                    ac.reasoning = f.engine.reasoning.clone();
                }
                if let Some(c) = f.engine.step_cap {
                    ac.step_cap = c;
                    config.governor.step_cap = c;
                }
                if let Some(t) = f.engine.timeout_s {
                    ac.timeout_secs = t;
                }
                let engine = AgentEngine::new(ac, clock.clone())
                    .map_err(|e| refuse(format!("engine: {e}")))?;
                let lister = f
                    .listing
                    .unwrap_or(true)
                    .then(|| Arc::new(HttpLister::new(&base)) as Arc<dyn Lister>);
                (Arc::new(engine), lister)
            }
            "mock" => {
                let mock = crate::sim::MockEngine::new(
                    crate::sim::MockConfig::default(),
                    clock.clone(),
                    crate::sim::FaultInjector::new(Vec::new()),
                );
                (Arc::new(mock), None)
            }
            other => return Err(refuse(format!("engine.kind: unknown engine `{other}`"))),
        };
    let acp = match &f.acp {
        None => AcpPlan::None,
        Some(a) => match (&a.stdio, &a.http) {
            (Some(_), Some(_)) => return Err(refuse("acp: stdio or http, not both")),
            (Some(r), None) => AcpPlan::Stdio(parse_role("acp.stdio", r)?),
            (None, Some(addr)) => {
                let mut tokens = Vec::new();
                for (r, name) in &a.tokens {
                    let role = parse_role("acp.tokens", r)?;
                    tokens.push((env(&format!("acp.tokens.{r}"), name)?, role));
                }
                if tokens.is_empty() {
                    return Err(refuse("acp.tokens: http needs at least one role token"));
                }
                // One token, one role: a shared value would leave it with
                // whichever role came last. The value is never printed.
                for (i, (t, r)) in tokens.iter().enumerate() {
                    if let Some((_, other)) = tokens[..i].iter().find(|(u, _)| u == t) {
                        return Err(refuse(format!(
                            "acp.tokens: {other:?} and {r:?} share one token value"
                        )));
                    }
                }
                AcpPlan::Http {
                    addr: addr.clone(),
                    tokens,
                }
            }
            (None, None) => AcpPlan::None,
        },
    };
    let desk = match &f.desk {
        None => DecisionDesk::rule_only(),
        Some(d) => desk_of(d)?,
    };
    let now = clock.now();
    let calendar = f
        .calendar
        .iter()
        .map(|c| Entry {
            id: c.id.clone(),
            when: When::At(now.saturating_add(c.in_s.saturating_mul(SECOND))),
            origin: Origin::Owner,
            text: c.text.clone(),
            firm: c.firm,
            missed: Missed::OnceLate,
        })
        .collect();
    let plan = Plan {
        state: f.state.clone(),
        config,
        clock: clock.clone(),
        engine,
        lister,
        listing: None,
        memory: f.memory.unwrap_or(true),
        limits: Limits {
            max_turns,
            until: f
                .run_for_s
                .map(|s| now.saturating_add(s.saturating_mul(SECOND))),
        },
        acp,
        desk,
        inbox: f.inbox.clone(),
        stop_file: f.stop_file.clone(),
        calendar,
    };
    Ok(Configured::new(
        plan,
        Carry {
            started: clock.now(),
        },
    ))
}

/// The desk the file asks for. A decider's key is read from the env var
/// it names; `rule_only` needs no decider.
fn desk_of(d: &DeskFile) -> Result<DecisionDesk, Refusal> {
    let mode = match d.mode.as_str() {
        "decide" => DeskMode::Decide,
        "shadow" => DeskMode::Shadow,
        "rule_only" => DeskMode::RuleOnly,
        other => return Err(refuse(format!("desk.mode: unknown mode `{other}`"))),
    };
    let mut desk = match d.decider.as_deref() {
        None if mode == DeskMode::RuleOnly => DecisionDesk::rule_only(),
        None => {
            return Err(refuse(
                "desk.decider: required unless desk.mode is rule_only",
            ));
        }
        Some("jev") => {
            let key_env = d
                .api_key_env
                .clone()
                .ok_or_else(|| refuse("desk.api_key_env: required for the jev decider"))?;
            let key = env("desk.api_key_env", &key_env)?;
            let base = d
                .base_url
                .clone()
                .unwrap_or_else(|| rung_std::decide::DEFAULT_BASE_URL.into());
            let model = d
                .model
                .clone()
                .unwrap_or_else(|| rung_std::decide::DEFAULT_MODEL.into());
            let jev =
                rung_std::decide::JevDecider::new(&base, &key, &model, crate::desk::ASK_TIMEOUT);
            DecisionDesk::new(Some(Arc::new(jev)), "jev", mode)
        }
        Some(other) => return Err(refuse(format!("desk.decider: unknown decider `{other}`"))),
    };
    let mut cap = SpendCap::default();
    for (field, v, slot) in [
        ("desk.cap_usd_day", d.cap_usd_day, &mut cap.per_day),
        ("desk.cap_usd_ask", d.cap_usd_ask, &mut cap.per_ask),
    ] {
        if let Some(v) = v {
            if !(v.is_finite() && v >= 0.0) {
                return Err(refuse(format!("{field}: not a non-negative amount")));
            }
            *slot = v;
        }
    }
    desk.cap = cap;
    desk.kill_file = d.kill_file.clone();
    Ok(desk)
}

ladder!(Startup {
    carry {
        started: crate::clock::Millis,
    }

    Configured(Plan) => Listed(Plan) => Recovered(Opening) => {
          Handed(Handoff)
        | Refused(Refusal)
    }
} impl {
    listed = |configured| {
        let carry = configured.carry().clone();
        let mut plan = configured.payload;
        if let Some(lister) = &plan.lister {
            let now = plan.clock.now();
            plan.listing = Some(crate::ladder::list(lister.as_ref(), &plan.config.ladder, &[], now));
        }
        Listed::new(plan, carry)
    },
    recovered = |listed| {
        let carry = listed.carry().clone();
        let plan = listed.payload;
        let mut b = HostBuilder::new(plan.config, &plan.state, plan.clock, plan.engine);
        stop_install();
        b.stop = Arc::new(StopAuthority::new(plan.stop_file, true));
        b.desk = plan.desk;
        b.seed_calendar = plan.calendar;
        let inbox = match &plan.inbox {
            Some(dir) => DirSource::new(dir)
                .map(|src| vec![Box::new(src) as Box<dyn Source>])
                .map_err(|e| format!("inbox {}: {e}", dir.display())),
            None => Ok(Vec::new()),
        };
        b.notifier = Notifier::from_env();
        b.lister = plan.lister;
        b.initial_listing = plan.listing;
        b.limits = plan.limits;
        if plan.memory {
            b.memory = Some(Arc::new(MemoryHost::baseline(&plan.state.join("memory"), "host")));
        }
        let opened = inbox.and_then(|sources| {
            b.sources = sources;
            Host::open(b).map_err(|e| format!("{}: {e}", plan.state.display()))
        });
        Recovered::new(Opening { opened, acp: plan.acp }, carry)
    },
    step = |recovered| {
        let Opening { opened, acp } = recovered.payload;
        Ok(match opened {
            Ok((host, woken)) => StepOutcome::Handed(Handed::new(Handoff {
                host,
                recovered: woken,
                acp,
            })),
            Err(why) => StepOutcome::Refused(Refused::new(Refusal {
                stage: "recovered",
                why,
            })),
        })
    },
});

fn stop_install() {
    crate::stop::install_signals();
}

/// Walk a configured start to its verdict.
pub fn start(configured: Configured) -> Result<Handoff, Refusal> {
    let listed = startup::listed(configured);
    let recovered = startup::recovered(listed);
    match startup::step(recovered) {
        Ok(startup::StepOutcome::Handed(h)) => Ok(h.into_payload()),
        Ok(startup::StepOutcome::Refused(r)) => Err(r.into_payload()),
        Err(f) => Err(Refusal {
            stage: "recovered",
            why: f.error.to_string(),
        }),
    }
}

pub use startup::{
    Carry, Configured, Handed, Listed, Recovered, Refused, StepOutcome, listed, recovered, step,
};

/// How a run ended.
#[derive(Debug)]
pub enum Ended {
    /// The stop authority halted the host.
    Halted(Why),
    /// ACP outward could not be served (the host was stopped for it).
    AcpFailed(String),
}

impl Handoff {
    /// Run the Presence loop (and ACP outward, when configured) until the
    /// stop authority halts it.
    pub fn run(self) -> Ended {
        let Handoff {
            host,
            recovered,
            acp,
        } = self;
        if matches!(acp, AcpPlan::None) {
            return Ended::Halted(host.run(recovered));
        }
        let bridge = crate::acp::Acp::attach(host.clone());
        let failed = Arc::new(AtomicBool::new(false));
        let err = Arc::new(std::sync::Mutex::new(String::new()));
        let (f2, e2, h2) = (failed.clone(), err.clone(), host.clone());
        let looped = std::thread::spawn(move || host.run(recovered));
        std::thread::spawn(move || match acp {
            // A listener that cannot serve stops the host.
            AcpPlan::Http { addr, tokens } => {
                if let Err(e) = crate::acp::serve_http(bridge, &addr, tokens) {
                    *crate::core::lock(&e2) = e;
                    f2.store(true, Ordering::SeqCst);
                    h2.core.stop.request(Why::Stopped { by: "acp".into() });
                }
            }
            // A stdio client that went away does not: the host runs on.
            AcpPlan::Stdio(role) => {
                if let Err(e) = crate::acp::serve_stdio(bridge, crate::acp::Principal { role }) {
                    eprintln!("rung-host: acp: {e}");
                }
            }
            AcpPlan::None => {}
        });
        let why = looped.join();
        // Let the bridge answer what the halt left open.
        std::thread::sleep(std::time::Duration::from_millis(300));
        if failed.load(Ordering::SeqCst) {
            return Ended::AcpFailed(crate::core::lock(&err).clone());
        }
        match why {
            Ok(w) => Ended::Halted(w),
            Err(_) => Ended::AcpFailed("the loop thread panicked".into()),
        }
    }
}
