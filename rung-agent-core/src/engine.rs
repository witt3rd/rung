//! The turn engine: one capped agent-loop turn, then the turn check, on a
//! thread the caller assembled.
//!
//! [`Engine`] holds what lasts across turns: the model settings, the tool
//! roster, a session-lived MCP roster (reconnected when a server is gone) and
//! the turn check's gate. It is built from values ([`EngineSpec`]); nothing in
//! [`Engine::turn`] reads the environment or a config file.
//!
//! [`Engine::turn`] takes a [`Thread`] and a [`TurnCtl`] and hands back a
//! [`TurnReport`]. It has no session-store side effects and never moves the
//! process cwd: loading and saving a session, isolation worktrees, recall and
//! retain are the caller's (the CLI's are in [`crate::run::run_job_ex`]).
//! Tools still resolve paths against the process cwd, as they always have.
//!
//! Diagnostics during the turn go to [`TurnCtl::sink`] ([`Stderr`] unless
//! the caller sets one). A turn stopped by the provider carries a typed
//! [`ProviderFailure`].

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use rung_std::agent::{self, FailureKind, LoopState, Thread, agentloop};
use rung_std::llm::{ChatMessage, LlmConfig, StreamEvent, StreamListener, Usage};
use rung_std::tools::{MAX_DEPTH, Spawn, Task, ToolCollection, ToolOutput, ToolRoster, Toolset};
use serde_json::{Value, json};

pub use rung_std::agent::{ProviderClass, ProviderFailure};
pub use rung_std::events::{Event, EventSink, Stderr};

use crate::args::Args;
use crate::catalog::{Kind, Scope};
use crate::config::TurnCheckSettings;
use crate::mcp::{McpRoster, McpSpec, WithMcp};
use crate::run::{Status, WrapTools};
use crate::turn_check::{self, Gate, Turn, TurnCheckReport, turncheck};

// ─── Spec ────────────────────────────────────────────────────────────────────

/// Everything an [`Engine`] is built from, as values.
#[derive(Debug, Clone)]
pub struct EngineSpec {
    /// The model and how to reach it. A stream listener here is ignored: the
    /// listener is per turn ([`TurnCtl::stream_listener`]).
    pub llm: LlmConfig,
    /// The catalog kind; with `max_iterations` it sets the turn's cap.
    pub kind: Kind,
    /// The tool groups the turn may call.
    pub scope: Scope,
    /// Model calls per turn; `None` takes the kind's default.
    pub max_iterations: Option<u32>,
    pub turn_check: TurnCheckSettings,
    /// The model takes images, so tool images are sent to it.
    pub tool_images: bool,
    /// MCP servers, connected once when the engine is built.
    pub mcp: Vec<McpSpec>,
    /// Where the `python` group keeps its sandbox (`<workspace>/.rung/python`).
    pub workspace: PathBuf,
}

impl EngineSpec {
    /// The CLI's values: `config.yaml` and `RUNG_*` env for the model, the
    /// turn check and tool images; `--tools` / `--toolset`, config `tools:`,
    /// `--max-iterations` and `--mcp-http` from `args`. `workspace` is the
    /// session's cwd.
    pub fn from_env(args: &Args, workspace: &Path) -> Result<Self, String> {
        let mut llm = crate::config::load()?;
        reasoning_from_env(&mut llm);
        let turn_check = crate::config::load_turn_check()?;
        let tool_images = crate::config::load_tool_images()?;
        let scope = resolve_scope(args)?;
        Ok(Self {
            llm,
            kind: args.kind,
            scope,
            max_iterations: args.max_iterations,
            turn_check,
            tool_images,
            mcp: args.mcp.clone(),
            workspace: workspace.to_path_buf(),
        })
    }

    /// Model calls a turn may make.
    pub fn cap(&self) -> u32 {
        self.kind.iteration_cap(self.max_iterations)
    }
}

/// `RUNG_REASONING` (e.g. "medium") when the settings name no reasoning
/// level. It maps to reasoning effort / a thinking budget.
pub fn reasoning_from_env(llm: &mut LlmConfig) {
    if llm.reasoning_level.is_none() {
        llm.reasoning_level = std::env::var("RUNG_REASONING")
            .ok()
            .filter(|s| !s.trim().is_empty());
    }
}

/// The tool scope: `--tools`, else config `tools:`, else the kind's preset.
pub fn resolve_scope(args: &Args) -> Result<Scope, String> {
    if let Some(spec) = &args.tools {
        return Scope::parse(spec);
    }
    if let Some(list) = crate::config::load_tool_groups()?
        && !list.is_empty()
    {
        return Scope::from_config_list(&list);
    }
    Ok(Scope::from_kind(args.kind))
}

// ─── Engine ──────────────────────────────────────────────────────────────────

/// One agent, across turns.
pub struct Engine {
    llm: LlmConfig,
    cap: u32,
    tool_images: bool,
    gate: Gate,
    /// The roster, python, `task` and any outer layer; MCP is added per turn.
    base: Arc<dyn Toolset>,
    mcp_specs: Vec<McpSpec>,
    mcp: Mutex<Option<Arc<McpRoster>>>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("model", &self.llm.model)
            .field("cap", &self.cap)
            .field("base", &self.base)
            .finish()
    }
}

/// An [`Engine`] whose roster is built and whose MCP servers are not yet
/// connected ([`EngineBuilder::connect`]).
pub struct EngineBuilder {
    spec: EngineSpec,
    roster: ToolRoster,
    outer: Option<Arc<dyn Toolset>>,
}

impl Engine {
    /// Build the roster and connect the MCP servers.
    pub fn new(spec: EngineSpec) -> Result<Self, String> {
        Self::build(spec)?.connect()
    }

    /// Build the roster: the scope's groups, and the python sandbox when the
    /// scope allows `python`.
    pub fn build(spec: EngineSpec) -> Result<EngineBuilder, String> {
        let mut roster: ToolRoster = spec.scope.roster();
        if spec.scope.allows_python() {
            let dir = spec.workspace.join(".rung").join("python");
            let sb = rung_std::python::Sandbox::open(rung_std::python::SandboxConfig::in_dir(&dir))
                .map_err(|e| format!("python sandbox: {e}"))?;
            roster.add(sb.collection());
        }
        Ok(EngineBuilder {
            spec,
            roster,
            outer: None,
        })
    }

    /// The model settings turns run with.
    pub fn llm(&self) -> &LlmConfig {
        &self.llm
    }

    /// Model calls a turn may make.
    pub fn cap(&self) -> u32 {
        self.cap
    }

    /// Connect the MCP servers again, replacing the roster. A turn does this
    /// itself when a server is gone.
    pub fn reconnect_mcp(&self) -> Result<(), String> {
        if self.mcp_specs.is_empty() {
            return Ok(());
        }
        let fresh = connect_mcp(&self.mcp_specs)?;
        let old = std::mem::replace(&mut *self.mcp.lock().expect("mcp roster"), fresh);
        if let Some(old) = old {
            old.abort();
        }
        Ok(())
    }

    /// The MCP roster for this turn, reconnected first when a server is gone.
    fn mcp_for_turn(&self) -> Option<Arc<McpRoster>> {
        let current = self.mcp.lock().expect("mcp roster").clone();
        if let Some(roster) = &current
            && !roster.alive()
        {
            match self.reconnect_mcp() {
                Ok(()) => rung_std::events::emit(
                    "rung-agent",
                    "mcp.reconnect",
                    "[rung-agent] mcp: a server was gone; reconnected",
                ),
                Err(e) => rung_std::events::emit(
                    "rung-agent",
                    "mcp.reconnect_failed",
                    &format!("[rung-agent] mcp: a server was gone; reconnect failed ({e})"),
                ),
            }
            return self.mcp.lock().expect("mcp roster").clone();
        }
        current
    }

    fn carry(
        &self,
        tools: Arc<dyn Toolset>,
        config: LlmConfig,
        cancel: Option<Arc<AtomicBool>>,
    ) -> agentloop::Carry {
        agentloop::Carry {
            state: LoopState {
                cancel,
                tool_images: self.tool_images,
                ..LoopState::new(self.cap, self.cap)
            },
            tools,
            config,
            python: None,
        }
    }

    /// Run one turn on `thread`: the agent loop to its end, then the turn
    /// check (with its one nudge) while the check is on.
    pub fn turn(&self, thread: Thread, ctl: TurnCtl) -> TurnReport {
        let _sink = rung_std::events::install(ctl.sink.clone());
        // A caller that already set the session's cancel flag (the CLI does,
        // for the whole job) keeps it; otherwise the turn's is set for it.
        let _cancel = (!crate::mcp::has_session_cancel())
            .then(|| crate::mcp::scope_session_cancel(ctl.cancel.clone()));

        let mut tools = self.base.clone();
        if let Some(mcp) = self.mcp_for_turn() {
            mcp.set_cancel(ctl.cancel.clone());
            tools = Arc::new(WithMcp { inner: tools, mcp });
        }
        if let ToolGate::Disabled(names) = &ctl.gate {
            tools = Arc::new(Gated {
                inner: tools,
                disabled: names.clone(),
            });
        }
        for wrap in &ctl.wrap_tools {
            tools = wrap(tools);
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut config = self.llm.clone();
        config.stream_listener = ctl.stream_listener.clone().map(|inner| {
            Arc::new(CallRecorder {
                inner,
                calls: calls.clone(),
            }) as Arc<dyn StreamListener>
        });

        let sent = thread.messages.len();
        let system_text = thread.system_prompt.clone();
        let rerun_failure = RefCell::new(None);
        let outcome = match agent::run(
            thread,
            self.carry(tools.clone(), config.clone(), ctl.cancel.clone()),
        ) {
            Ok(r) => {
                let first_calls = r.api_calls_made;
                let first_elided = r.elided;
                let rerun = |messages: Vec<ChatMessage>| {
                    let thread = Thread {
                        system_prompt: system_text.clone(),
                        messages,
                    };
                    let out = agent::run(
                        thread,
                        self.carry(tools.clone(), config.clone(), ctl.cancel.clone()),
                    );
                    if let Err(f) = &out {
                        *rerun_failure.borrow_mut() = ProviderFailure::of(f);
                    }
                    out
                };
                let ended = settle(&self.gate, r, sent, &ctl.request, &ctl.earlier, rerun);
                let Ended {
                    result,
                    status,
                    report,
                    extra_calls,
                } = ended;
                // A nudge re-run is a second loop with its own elision.
                let elided = first_elided + if extra_calls > 0 { result.elided } else { 0 };
                Ok(TurnDone {
                    result,
                    status,
                    turn_check: report,
                    api_calls: first_calls + extra_calls,
                    elided,
                })
            }
            Err(f) => Err(f),
        };
        let failure = match &outcome {
            Err(f) => ProviderFailure::of(f),
            Ok(_) => rerun_failure.into_inner(),
        };
        let calls = std::mem::take(&mut *calls.lock().expect("calls"));
        TurnReport {
            outcome,
            failure,
            calls,
        }
    }
}

impl EngineBuilder {
    /// Give the roster a `task` tool backed by `spawn`, when the scope
    /// allows `task`.
    pub fn with_task(mut self, spawn: Arc<dyn Spawn>) -> Self {
        if self.spec.scope.allows_task() {
            let mut tasks = ToolCollection::new("task");
            tasks.admit(Task::new(spawn, 0, MAX_DEPTH));
            self.roster.add(tasks);
        }
        self
    }

    /// Layer `outer` over the roster (a memory provider's tools); MCP tools
    /// go over both.
    pub fn with_outer(mut self, outer: Option<Arc<dyn Toolset>>) -> Self {
        self.outer = outer;
        self
    }

    /// Connect the MCP servers and finish the engine.
    pub fn connect(self) -> Result<Engine, String> {
        let EngineBuilder {
            spec,
            roster,
            outer,
        } = self;
        let mut base: Arc<dyn Toolset> = Arc::new(roster);
        if let Some(outer) = outer {
            base = Arc::new(crate::memory::Layered { inner: base, outer });
        }
        let mcp = if spec.mcp.is_empty() {
            None
        } else {
            connect_mcp(&spec.mcp)?
        };
        let gate = Gate::from_settings(&spec.turn_check);
        let cap = spec.cap();
        Ok(Engine {
            llm: spec.llm,
            cap,
            tool_images: spec.tool_images,
            gate,
            base,
            mcp_specs: spec.mcp,
            mcp: Mutex::new(mcp),
        })
    }
}

/// A roster that holds tools, or `None` when the servers offer none.
fn connect_mcp(specs: &[McpSpec]) -> Result<Option<Arc<McpRoster>>, String> {
    let mcp = McpRoster::connect(specs)?;
    Ok((!mcp.is_empty()).then(|| Arc::new(mcp)))
}

// ─── Turn control and report ─────────────────────────────────────────────────

/// Which declared tools a turn may run. A disabled tool stays declared (its
/// definition is still sent, so a cached prefix holds) and a call to it is
/// refused without running.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ToolGate {
    #[default]
    AllowAll,
    Disabled(BTreeSet<String>),
}

/// What one turn is given besides its thread.
#[derive(Clone)]
pub struct TurnCtl {
    /// Checked before each LLM call and around each tool.
    pub cancel: Option<Arc<AtomicBool>>,
    /// Model stream events. Set, the model is called streaming.
    pub stream_listener: Option<Arc<dyn StreamListener>>,
    /// Wrappers over the turn's tools, innermost first (observers).
    pub wrap_tools: Vec<WrapTools>,
    /// Where the turn's diagnostics go.
    pub sink: Arc<dyn EventSink>,
    pub gate: ToolGate,
    /// What the user asked this turn, for the turn check.
    pub request: String,
    /// Earlier turns' assistant messages, for the turn check's prior actions.
    pub earlier: Vec<ChatMessage>,
}

impl Default for TurnCtl {
    fn default() -> Self {
        Self {
            cancel: None,
            stream_listener: None,
            wrap_tools: Vec::new(),
            sink: Arc::new(Stderr),
            gate: ToolGate::AllowAll,
            request: String::new(),
            earlier: Vec::new(),
        }
    }
}

/// One LLM call as the provider served it (from the stream events).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CallUsage {
    /// The model the provider says served the call.
    pub model: String,
    /// The call's usage, `provider` included, when the provider reported it.
    pub usage: Option<Usage>,
}

/// A turn that ended with an answer (checked or not).
#[derive(Debug)]
pub struct TurnDone {
    /// The loop's result to keep (after a nudge, the second loop's; after a
    /// nudge that stopped, the first answer with the re-run's steps).
    pub result: agent::AgentResult,
    pub status: Status,
    /// The turn check's reading; `None` while the check is off.
    pub turn_check: Option<TurnCheckReport>,
    /// Model calls, the nudge re-run's included.
    pub api_calls: u32,
    /// Tool results elided after a context overflow, both loops counted.
    pub elided: usize,
}

/// What one turn did.
#[derive(Debug)]
pub struct TurnReport {
    /// The answer, or why the loop stopped without one.
    pub outcome: Result<TurnDone, agent::Filtered>,
    /// The provider failure that stopped the turn (or its nudge re-run).
    pub failure: Option<ProviderFailure>,
    /// One entry per LLM call, recorded while the turn streams; empty when
    /// it does not ([`TurnCtl::stream_listener`] unset).
    pub calls: Vec<CallUsage>,
}

/// Tees stream events into [`CallUsage`] records.
struct CallRecorder {
    inner: Arc<dyn StreamListener>,
    calls: Arc<Mutex<Vec<CallUsage>>>,
}

impl StreamListener for CallRecorder {
    fn on_event(&self, event: StreamEvent) {
        match &event {
            StreamEvent::MessageStart { model, .. } => {
                self.calls.lock().expect("calls").push(CallUsage {
                    model: model.clone(),
                    usage: None,
                });
            }
            StreamEvent::MessageDelta {
                usage: Some(usage), ..
            } => {
                if let Some(last) = self.calls.lock().expect("calls").last_mut() {
                    last.usage = Some(usage.clone());
                }
            }
            _ => {}
        }
        self.inner.on_event(event);
    }

    fn on_http_failure(&self, failure: &rung_std::llm::HttpFailure) {
        self.inner.on_http_failure(failure);
    }
}

/// Refuses calls to disabled tools; passes the rest through.
struct Gated {
    inner: Arc<dyn Toolset>,
    disabled: BTreeSet<String>,
}

impl std::fmt::Debug for Gated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gated")
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl Gated {
    fn refusal(&self, name: &str) -> Option<String> {
        self.disabled.contains(name).then(|| {
            json!({
                "refused": name,
                "reason": "this tool is disabled for this turn; it did not run",
            })
            .to_string()
        })
    }
}

impl Toolset for Gated {
    fn definitions(&self) -> Vec<rung_std::llm::ToolDefinition> {
        self.inner.definitions()
    }

    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        match self.refusal(name) {
            Some(r) => Err(r),
            None => self.inner.execute(name, input),
        }
    }

    fn execute_output(&self, name: &str, input: &Value) -> Result<ToolOutput, String> {
        match self.refusal(name) {
            Some(r) => Err(r),
            None => self.inner.execute_output(name, input),
        }
    }
}

// ─── The turn check ──────────────────────────────────────────────────────────

/// A turn after its check: the result to persist and report, its status, and
/// the reading. `extra_calls` counts the nudge re-run's model calls.
pub(crate) struct Ended {
    pub(crate) result: agent::AgentResult,
    pub(crate) status: Status,
    pub(crate) report: Option<TurnCheckReport>,
    pub(crate) extra_calls: u32,
}

/// Settle a loop that ran to its end: truncated, completed unjudged while the
/// check is off, else the TurnCheck ladder's verdict. A top-level turn and a
/// nested `task` child ([`crate::run::CatalogSpawn`]) both end here, so a
/// child is never `completed` on its own say-so while the check is on.
///
/// `sent` is how many messages the loop was given; `request` and `earlier`
/// are what the check reads; `rerun` runs the loop again for the one nudge.
pub(crate) fn settle(
    gate: &Gate,
    r: agent::AgentResult,
    sent: usize,
    request: &str,
    earlier: &[ChatMessage],
    rerun: impl Fn(Vec<ChatMessage>) -> Result<agent::AgentResult, agent::Filtered>,
) -> Ended {
    if r.truncated {
        return Ended {
            result: r,
            status: Status::Truncated,
            report: None,
            extra_calls: 0,
        };
    }
    match gate {
        Gate::Off(off) => Ended {
            result: r,
            status: Status::Completed(off.completion()),
            report: None,
            extra_calls: 0,
        },
        Gate::On(decider) => {
            let carry = turncheck::Carry {
                decider: decider.clone(),
                request: request.to_string(),
                prior_actions: turn_check::prior_actions(earlier),
            };
            check_turn(Turn::first(r, sent), carry, rerun)
        }
    }
}

/// Drive the TurnCheck ladder: check, nudge once and check again if the turn
/// narrated, then stop. `rerun` runs the agent loop on the given messages.
fn check_turn(
    turn: Turn,
    carry: turncheck::Carry,
    rerun: impl Fn(Vec<ChatMessage>) -> Result<agent::AgentResult, agent::Filtered>,
) -> Ended {
    let done = |result, status, report: &TurnCheckReport, extra_calls| Ended {
        result,
        status,
        report: Some(report.clone()),
        extra_calls,
    };
    let first = turncheck::Ended::new(turn, carry.clone());
    let nudged = match turncheck::step(first) {
        Ok(turncheck::StepOutcome::Completed(c)) => {
            let c = c.into_payload();
            let status = Status::Completed(c.completion());
            let report = c.report().clone();
            return done(c.into_result(), status, &report, 0);
        }
        Ok(turncheck::StepOutcome::Unverified(f)) => {
            let f = f.into_payload();
            let report = f.report().clone();
            return done(f.into_result(), Status::Unverified, &report, 0);
        }
        Ok(turncheck::StepOutcome::Unchecked(u)) => {
            let u = u.into_payload();
            let report = u.report().clone();
            return done(u.into_result(), Status::Unchecked, &report, 0);
        }
        Ok(turncheck::StepOutcome::Nudge(n)) => n.into_payload(),
        Err(f) => {
            // The step never fails; a failure would still not be a completion.
            let t = f.token.payload;
            return Ended {
                result: t.into_result(),
                status: Status::Unchecked,
                report: None,
                extra_calls: 0,
            };
        }
    };
    rung_std::events::emit(
        "rung-agent",
        "turncheck.nudge",
        "[rung-agent] turn check: the turn narrated; nudging once",
    );
    let second = match rerun(nudged.rerun_messages()) {
        Ok(r) => r,
        Err(e) => {
            let status = if e.kind == FailureKind::Interrupted {
                Status::Cancelled
            } else {
                Status::Unverified
            };
            let f = nudged.rerun_stopped(e);
            let report = f.report().clone();
            return done(f.into_result(), status, &report, 0);
        }
    };
    let extra = second.api_calls_made;
    if second.truncated {
        let report = TurnCheckReport {
            nudged: true,
            reading: Some(nudged.reading().clone()),
            reason: None,
        };
        return done(second, Status::Truncated, &report, extra);
    }
    let again = turncheck::Ended::new(Turn::after_nudge(nudged, second), carry);
    match turncheck::step(again) {
        Ok(turncheck::StepOutcome::Completed(c)) => {
            let c = c.into_payload();
            let status = Status::Completed(c.completion());
            let report = c.report().clone();
            done(c.into_result(), status, &report, extra)
        }
        Ok(turncheck::StepOutcome::Unchecked(u)) => {
            let u = u.into_payload();
            let report = u.report().clone();
            done(u.into_result(), Status::Unchecked, &report, extra)
        }
        Ok(turncheck::StepOutcome::Unverified(f)) => {
            let f = f.into_payload();
            let report = f.report().clone();
            done(f.into_result(), Status::Unverified, &report, extra)
        }
        // The gate never nudges a nudged turn; if it did, still no completion.
        Ok(turncheck::StepOutcome::Nudge(n)) => {
            let f = n.into_payload().into_flagged();
            let report = f.report().clone();
            done(f.into_result(), Status::Unverified, &report, extra)
        }
        Err(f) => Ended {
            result: f.token.payload.into_result(),
            status: Status::Unchecked,
            report: None,
            extra_calls: extra,
        },
    }
}
