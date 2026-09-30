//! Drive [`rung_std::agent::run`] with a catalog roster, optional nested
//! [`CatalogSpawn`], and a session file.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rung_std::agent::{self, FailureKind, LoopState, Thread, agentloop};
use rung_std::llm::{ChatMessage, LlmConfig, MessageContent, MessageContentBlock};
use rung_std::tools::{
    MAX_DEPTH, Spawn, Task, TaskRequest, TaskResult, ToolCollection, ToolRoster, Toolset,
    WithoutTask,
};

use serde::{Serialize, Serializer};

use crate::args::{Args, IsolationMode};
use crate::catalog::{Kind, Scope};
use crate::session::{Line, Session, SessionStore};
use crate::turn_check::{self, Completion, Gate, Turn, TurnCheckReport, turncheck};

/// How a job ended. Serialised as the status string hosts already read, plus
/// `unverified` and `unchecked` from the turn check.
///
/// `Completed` holds a [`Completion`], which only a passed check or the
/// switched-off check can make: nothing here can call a turn completed on the
/// model's say-so while the check is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Completed(Completion),
    /// Cut off by the token limit (`finish_reason: length`).
    Truncated,
    /// The judge read the turn and did not pass it.
    Unverified,
    /// The check was on and no reading came back.
    Unchecked,
    Cancelled,
    /// Started in the background.
    Running,
    /// Read back from the session file (poll); not a judgment made now.
    Recorded(String),
}

impl Status {
    pub fn as_str(&self) -> &str {
        match self {
            Status::Completed(_) => "completed",
            Status::Truncated => "truncated",
            Status::Unverified => "unverified",
            Status::Unchecked => "unchecked",
            Status::Cancelled => "cancelled",
            Status::Running => "running",
            Status::Recorded(s) => s,
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Status {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub task_id: String,
    pub text: String,
    pub status: Status,
    pub api_calls: u32,
    pub isolation_path: Option<String>,
    /// The turn check's reading. Absent while the check is off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_check: Option<TurnCheckReport>,
}

/// Wrap a roster so execute is observed (ACP tool-call updates).
pub type WrapTools = Arc<dyn Fn(Arc<dyn Toolset>) -> Arc<dyn Toolset> + Send + Sync>;

/// Optional cancel + tool wrap for one job (ACP).
#[derive(Clone, Default)]
pub struct JobEx {
    pub cancel: Option<Arc<AtomicBool>>,
    pub wrap_tools: Option<WrapTools>,
    /// Replace the last user message with these blocks (ACP image/audio).
    pub prompt_blocks: Option<Vec<MessageContentBlock>>,
    /// Forward model stream events (thinking deltas) to the ACP client.
    pub stream_listener: Option<Arc<dyn rung_std::llm::StreamListener>>,
    /// Per-session system text (ACP `session/new` `_meta.systemPrompt`),
    /// appended after the process system prompt.
    pub system_append: Option<String>,
}

/// Nested `task` Spawn: pick a catalog kind, persist a child session, run a
/// depth-capped loop. Isolation stays the process cwd (parent already chdir'd).
#[derive(Clone)]
pub struct CatalogSpawn {
    pub config: LlmConfig,
    pub store: SessionStore,
    pub max_iterations: u32,
    pub emitter: Option<Arc<crate::stream::Emitter>>,
    pub extra: JobEx,
}

impl std::fmt::Debug for CatalogSpawn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CatalogSpawn")
            .field("store", &self.store.dir)
            .field("max_iterations", &self.max_iterations)
            .finish()
    }
}

impl Spawn for CatalogSpawn {
    fn spawn(&self, req: &TaskRequest) -> Result<TaskResult, String> {
        let kind = match &req.subagent_type {
            Some(s) => Kind::parse(s)?,
            None => Kind::Explore,
        };
        let id = match &req.task_id {
            Some(id) => {
                crate::session::check_id(id)?;
                id.clone()
            }
            None => crate::session::new_id(),
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let _cancel_guard = crate::mcp::set_session_cancel(self.extra.cancel.clone());
        let _sink_guard = crate::mcp::set_session_sink(Some((self.store.dir.clone(), id.clone())));
        let mut sess = match self.store.try_load(&id)? {
            Some(s) => s,
            None => Session::new(&id, kind, &cwd),
        };
        sess.kind = kind.as_str().into();
        sess.lines.push(Line::user(req.prompt.clone()));
        sess.status = "running".into();
        sess.pid = Some(std::process::id());
        self.store.save(&sess)?;
        match drive(
            &self.config,
            kind,
            &sess.lines,
            self.max_iterations,
            self.emitter.clone(),
            &self.extra,
        ) {
            Ok((line, api_calls)) => {
                let text = line.text.clone();
                sess.lines.push(line);
                sess.status = "completed".into();
                self.store.save(&sess)?;
                Ok(TaskResult {
                    text,
                    api_calls,
                    task_id: Some(id),
                })
            }
            Err(e) => {
                sess.status = "error".into();
                sess.lines.push(Line::assistant(e.clone()));
                let _ = self.store.save(&sess);
                Err(e)
            }
        }
    }
}

fn drive(
    config: &LlmConfig,
    kind: Kind,
    lines: &[Line],
    max_iterations: u32,
    emitter: Option<Arc<crate::stream::Emitter>>,
    extra: &JobEx,
) -> Result<(Line, u32), String> {
    let _cancel_guard = crate::mcp::set_session_cancel(extra.cancel.clone());
    let cap = max_iterations.min(kind.max_iterations()).max(1);
    let base: Arc<dyn Toolset> = Arc::new(WithoutTask::new(Arc::new(kind.roster())));
    let tools = wrap_tools(base, emitter.as_ref(), extra);
    let mut config = config.clone();
    if let Some(em) = &emitter {
        config.stream_listener = Some(em.clone() as Arc<dyn rung_std::llm::StreamListener>);
    }
    // The ACP prompt path forwards thinking and message deltas to the client.
    if config.stream_listener.is_none()
        && let Some(listener) = &extra.stream_listener
    {
        config.stream_listener = Some(listener.clone());
    }
    // Reasoning visibility: RUNG_REASONING (e.g. "medium") maps to
    // reasoning_effort / thinking budget. GLM-class models emit
    // reasoning_content deltas regardless; this asks for them.
    if config.reasoning_level.is_none() {
        config.reasoning_level = std::env::var("RUNG_REASONING")
            .ok()
            .filter(|s| !s.trim().is_empty());
    }
    let thread = thread_from(lines, None, None);
    let state = LoopState {
        cancel: extra.cancel.clone(),
        ..LoopState::new(cap, cap)
    };
    let carry = agentloop::Carry {
        state,
        tools,
        config,
        python: None,
    };
    let sent = thread.messages.len();
    match agent::run(thread, carry) {
        Ok(r) => Ok((turn_line(&r, sent), r.api_calls_made)),
        Err(f) => Err(f.reason),
    }
}

fn wrap_tools(
    base: Arc<dyn Toolset>,
    emitter: Option<&Arc<crate::stream::Emitter>>,
    extra: &JobEx,
) -> Arc<dyn Toolset> {
    let tools: Arc<dyn Toolset> = match emitter {
        Some(em) => Arc::new(crate::stream::ObservingToolset {
            inner: base,
            emitter: em.clone(),
        }),
        None => base,
    };
    match &extra.wrap_tools {
        Some(w) => w(tools),
        None => tools,
    }
}

fn resolve_scope(args: &Args) -> Result<Scope, String> {
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

fn thread_from(lines: &[Line], system_text: Option<&str>, user_material: Option<&str>) -> Thread {
    let system_prompt = system_text.unwrap_or("").to_string();
    // User slot: prepared material becomes the FIRST user message, preceding
    // the session lines (which already end with the current ask).
    let mut messages = Vec::new();
    if let Some(m) = user_material {
        messages.push(ChatMessage::user(m));
    }
    for l in lines {
        match (l.role.as_str(), &l.messages) {
            ("user", _) => messages.push(ChatMessage::user(l.text.clone())),
            ("assistant", Some(turn)) if !turn.is_empty() => messages.extend(turn.iter().cloned()),
            ("assistant", _) => messages.push(ChatMessage::assistant(l.text.clone())),
            _ => {}
        }
    }
    Thread {
        system_prompt,
        messages,
    }
}

/// History cap for a replayed tool result. The call itself is kept whole.
const HISTORY_TOOL_RESULT_CHARS: usize = 4000;

/// The assistant line for a finished turn: the messages the loop added after
/// the `sent` messages it was given, with large tool results shortened.
fn turn_line(r: &agent::AgentResult, sent: usize) -> Line {
    let mut turn: Vec<ChatMessage> = r.transcript.iter().skip(sent).cloned().collect();
    for m in &mut turn {
        if let MessageContent::Blocks(blocks) = &mut m.content {
            for b in blocks {
                if let MessageContentBlock::ToolResult { content, .. } = b
                    && content.chars().count() > HISTORY_TOOL_RESULT_CHARS
                {
                    let kept: String = content.chars().take(HISTORY_TOOL_RESULT_CHARS).collect();
                    *content = format!("{kept}\n[… shortened in history]");
                }
            }
        }
    }
    Line {
        role: "assistant".into(),
        text: r.final_response.clone(),
        messages: Some(turn),
    }
}

fn last_assistant(lines: &[Line]) -> String {
    lines
        .iter()
        .rev()
        .find(|l| l.role == "assistant")
        .map(|l| l.text.clone())
        .unwrap_or_default()
}

/// System prompt precedence: explicit `--system-prompt` (TEXT or @file)
/// beats the env channel. `RUNG_SYSTEM_PROMPT_FILE` is rung's own config
/// surface: a host (its own wrapper, a fleet driver, an agent harness)
/// may place a system-prompt *file path* there at spawn, keeping child
/// argv purely the ACP entry flags.
fn resolve_system_prompt(
    origin: &Path,
    explicit: Option<&String>,
) -> Result<Option<String>, String> {
    match explicit {
        Some(s) => read_text(origin, s).map(Some),
        None => std::env::var("RUNG_SYSTEM_PROMPT_FILE")
            .ok()
            .filter(|p| !p.trim().is_empty())
            .map(|p| read_text(origin, &format!("@{p}")))
            .transpose(),
    }
}

fn read_text(origin: &Path, spec: &str) -> Result<String, String> {
    // Inline text unless the value starts with "@", in which case it names a
    // file (absolute, or relative to origin) whose bytes become the value.
    if let Some(path) = spec.strip_prefix('@') {
        let resolved = if Path::new(path).is_absolute() {
            Path::new(path).to_path_buf()
        } else {
            origin.join(path)
        };
        return std::fs::read_to_string(&resolved).map_err(|e| format!("{path}: {e}"));
    }
    Ok(spec.to_string())
}

fn pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

struct CwdGuard(PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

pub fn run_job(args: &Args, origin: &Path) -> Result<Outcome, String> {
    run_job_ex(args, origin, JobEx::default())
}

pub fn run_job_ex(args: &Args, origin: &Path, extra: JobEx) -> Result<Outcome, String> {
    let _cancel_guard = crate::mcp::set_session_cancel(extra.cancel.clone());
    if let Some(id) = &args.task_id {
        crate::session::check_id(id)?;
    }
    let origin = origin
        .canonicalize()
        .unwrap_or_else(|_| origin.to_path_buf());
    let store = SessionStore::in_cwd(&origin);
    let id = match &args.task_id {
        Some(id) => id.clone(),
        None => crate::session::new_id(),
    };
    let _sink_guard = crate::mcp::set_session_sink(Some((store.dir.clone(), id.clone())));

    if args.background && !crate::background::in_child() {
        let prompt = args
            .prompt
            .as_ref()
            .ok_or_else(|| "background needs a prompt".to_string())?;
        let mut sess = store
            .try_load(&id)?
            .unwrap_or_else(|| Session::new(&id, args.kind, &origin));
        sess.kind = args.kind.as_str().into();
        sess.status = "queued".into();
        sess.lines.push(Line::user(prompt.clone()));
        store.save(&sess)?;
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        let launch = crate::background::spawn_child(&exe, args, &origin, &id, &store)?;
        sess.pid = Some(launch.pid);
        sess.status = "running".into();
        store.save(&sess)?;
        return Ok(Outcome {
            task_id: id,
            text: format!("pid={} log={}", launch.pid, launch.log.display()),
            status: Status::Running,
            api_calls: 0,
            isolation_path: sess.isolation_path,
            turn_check: None,
        });
    }

    let mut sess = store
        .try_load(&id)?
        .unwrap_or_else(|| Session::new(&id, args.kind, &origin));

    if args.prompt.is_none() {
        if store.try_load(&id)?.is_none() {
            return Err(format!("no session {id}"));
        }
        if sess.status == "running" && sess.pid.is_some_and(|p| !pid_alive(p)) {
            sess.status = "interrupted".into();
            store.save(&sess)?;
        }
        return Ok(Outcome {
            task_id: id,
            text: last_assistant(&sess.lines),
            status: Status::Recorded(sess.status),
            api_calls: 0,
            isolation_path: sess.isolation_path.clone(),
            turn_check: None,
        });
    }

    let prompt = args.prompt.clone().unwrap();
    // New session or resume: don't duplicate the last identical user line
    // (background parent already wrote it).
    let already = sess
        .lines
        .last()
        .is_some_and(|l| l.role == "user" && l.text == prompt);
    if !already {
        sess.lines.push(Line::user(prompt));
    }
    sess.kind = args.kind.as_str().into();
    sess.pid = Some(std::process::id());

    let _guard = CwdGuard(origin.clone());
    if args.isolation == IsolationMode::Worktree {
        let wt = if let Some(p) = &sess.isolation_path {
            let path = PathBuf::from(p);
            if path.is_dir() {
                crate::isolation::Worktree {
                    path,
                    branch: format!("rung-task/{id}"),
                    created: false,
                }
            } else {
                crate::isolation::ensure(&id, &origin)?
            }
        } else {
            crate::isolation::ensure(&id, &origin)?
        };
        std::env::set_current_dir(&wt.path)
            .map_err(|e| format!("chdir {}: {e}", wt.path.display()))?;
        sess.isolation_path = Some(wt.path.to_string_lossy().into_owned());
    }

    sess.status = "running".into();
    store.save(&sess)?;

    let emitter = if args.stream {
        Some(crate::stream::Emitter::new())
    } else {
        None
    };

    let mut config = match crate::config::load() {
        Ok(c) => c,
        Err(e) => {
            sess.status = "error".into();
            sess.lines.push(Line::assistant(e.clone()));
            let _ = store.save(&sess);
            return Err(e);
        }
    };
    if let Some(em) = &emitter {
        config.stream_listener = Some(em.clone() as Arc<dyn rung_std::llm::StreamListener>);
    }
    // The ACP prompt path forwards thinking deltas to the client even
    // though the final text is sent at turn end.
    if config.stream_listener.is_none()
        && let Some(listener) = &extra.stream_listener
    {
        config.stream_listener = Some(listener.clone());
    }
    // Reasoning visibility: RUNG_REASONING (e.g. "medium") maps to
    // reasoning_effort / thinking budget. GLM-class models emit
    // reasoning_content deltas regardless; this asks for them.
    if config.reasoning_level.is_none() {
        config.reasoning_level = std::env::var("RUNG_REASONING")
            .ok()
            .filter(|s| !s.trim().is_empty());
    }
    let model = config.model.clone();
    let turn_check_gate = match crate::config::load_turn_check() {
        Ok(t) => Gate::from_settings(&t),
        Err(e) => {
            sess.status = "error".into();
            sess.lines.push(Line::assistant(e.clone()));
            let _ = store.save(&sess);
            return Err(e);
        }
    };

    let scope = resolve_scope(args)?;
    let cap = args
        .max_iterations
        .unwrap_or_else(|| args.kind.max_iterations())
        .max(1);
    let mut roster: ToolRoster = scope.roster();
    if scope.allows_python() {
        let dir = origin.join(".rung").join("python");
        let sb = rung_std::python::Sandbox::open(rung_std::python::SandboxConfig::in_dir(&dir))
            .map_err(|e| format!("python sandbox: {e}"))?;
        roster.add(sb.collection());
    }
    if scope.allows_task() {
        let spawn = CatalogSpawn {
            config: config.clone(),
            store: store.clone(),
            max_iterations: cap,
            emitter: emitter.clone(),
            extra: extra.clone(),
        };
        let mut tasks = ToolCollection::new("task");
        tasks.admit(Task::new(Arc::new(spawn), 0, MAX_DEPTH));
        roster.add(tasks);
    }
    let mut base: Arc<dyn Toolset> = Arc::new(roster);
    if !args.mcp.is_empty() {
        let mut mcp = crate::mcp::McpRoster::connect(&args.mcp)?;
        mcp.set_cancel(extra.cancel.clone());
        if !mcp.is_empty() {
            base = Arc::new(crate::mcp::WithMcp {
                inner: base,
                mcp: Arc::new(mcp),
            });
        }
    }
    let tools = wrap_tools(base, emitter.as_ref(), &extra);
    let system_prompt = resolve_system_prompt(&origin, args.system_prompt.as_ref())?;
    let system_prompt = match (system_prompt, extra.system_append.as_deref()) {
        (base, None) => base,
        (None, Some(add)) => Some(add.to_string()),
        (Some(base), Some(add)) => Some(format!("{base}\n\n{add}")),
    };
    let user_material = match &args.user_prompt {
        Some(u) => Some(read_text(&origin, u)?),
        None => None,
    };
    let mut thread = thread_from(
        &sess.lines,
        system_prompt.as_deref(),
        user_material.as_deref(),
    );
    if let Some(blocks) = extra.prompt_blocks.clone()
        && let Some(last) = thread.messages.last_mut()
        && last.role == "user"
    {
        last.content = MessageContent::Blocks(blocks);
    }
    let loop_carry = |tools: Arc<dyn Toolset>, config: LlmConfig| agentloop::Carry {
        state: LoopState {
            cancel: extra.cancel.clone(),
            ..LoopState::new(cap, cap)
        },
        tools,
        config,
        python: None,
    };
    let sent = thread.messages.len();
    let system_text = thread.system_prompt.clone();
    let request = args.prompt.clone().unwrap_or_default();
    let earlier: Vec<ChatMessage> = sess
        .lines
        .iter()
        .take(sess.lines.len().saturating_sub(1))
        .filter(|l| l.role == "assistant")
        .flat_map(|l| l.messages.clone().unwrap_or_default())
        .collect();
    let prior_actions = turn_check::prior_actions(&earlier);
    match agent::run(thread, loop_carry(tools.clone(), config.clone())) {
        Ok(r) => {
            let first_calls = r.api_calls_made;
            let ended = if r.truncated {
                Ended {
                    result: r,
                    status: Status::Truncated,
                    report: None,
                    extra_calls: 0,
                }
            } else {
                match turn_check_gate {
                    Gate::Off(off) => Ended {
                        result: r,
                        status: Status::Completed(off.completion()),
                        report: None,
                        extra_calls: 0,
                    },
                    Gate::On(decider) => {
                        let carry = turncheck::Carry {
                            decider,
                            request,
                            prior_actions,
                        };
                        let rerun = |messages: Vec<ChatMessage>| {
                            let thread = Thread {
                                system_prompt: system_text.clone(),
                                messages,
                            };
                            agent::run(thread, loop_carry(tools.clone(), config.clone()))
                        };
                        check_turn(Turn::first(r, sent), carry, rerun)
                    }
                }
            };
            let Ended {
                result: r,
                status,
                report,
                extra_calls,
            } = ended;
            sess.lines.push(turn_line(&r, sent));
            sess.status = match status {
                Status::Cancelled => "interrupted".into(),
                _ => status.as_str().into(),
            };
            store.save(&sess)?;
            let out = Outcome {
                task_id: id,
                text: r.final_response.clone(),
                status,
                api_calls: first_calls + extra_calls,
                isolation_path: sess.isolation_path,
                turn_check: report,
            };
            if let Some(em) = &emitter {
                em.emit_result(&out, &r.usage, &model);
            }
            Ok(out)
        }
        Err(f) if f.kind == FailureKind::Interrupted => {
            sess.status = "interrupted".into();
            let _ = store.save(&sess);
            Ok(Outcome {
                task_id: id,
                text: String::new(),
                status: Status::Cancelled,
                api_calls: 0,
                isolation_path: sess.isolation_path,
                turn_check: None,
            })
        }
        Err(f) => {
            sess.status = "error".into();
            sess.lines.push(Line::assistant(f.reason.clone()));
            let _ = store.save(&sess);
            if let Some(em) = &emitter {
                em.emit_error(&id, &f.reason, &model);
            }
            Err(f.reason)
        }
    }
}

/// A turn after its check: the result to persist and report, its status, and
/// the reading. `extra_calls` counts the nudge re-run's model calls.
struct Ended {
    result: agent::AgentResult,
    status: Status,
    report: Option<TurnCheckReport>,
    extra_calls: u32,
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
    eprintln!("[rung-agent] turn check: the turn narrated; nudging once");
    let second = match rerun(nudged.rerun_messages()) {
        Ok(r) => r,
        Err(e) => {
            let status = if e.kind == FailureKind::Interrupted {
                Status::Cancelled
            } else {
                Status::Unverified
            };
            let f = nudged.into_flagged();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_skips_non_chat_roles() {
        let lines = vec![
            Line {
                role: "system".into(),
                text: "nope".into(),
                messages: None,
            },
            Line::user("hi"),
        ];
        let t = thread_from(&lines, None, None);
        assert_eq!(t.messages.len(), 1);
        assert_eq!(t.system_prompt, "");
    }

    #[test]
    fn system_prompt_env_channel_and_precedence() {
        let dir = std::env::temp_dir().join(format!("rung-sys-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("prefill.md");
        std::fs::write(&f, "# packed profile").unwrap();
        unsafe { std::env::set_var("RUNG_SYSTEM_PROMPT_FILE", &f) };

        // env channel used when argv is silent
        let got = resolve_system_prompt(Path::new("/tmp"), None).unwrap();
        assert_eq!(got.as_deref(), Some("# packed profile"));

        // explicit argv wins over the env channel
        let got =
            resolve_system_prompt(Path::new("/tmp"), Some(&"@/etc/hostname".to_string())).unwrap();
        assert!(got.is_some());
        assert_ne!(got.as_deref(), Some("# packed profile"));

        unsafe { std::env::remove_var("RUNG_SYSTEM_PROMPT_FILE") };
        let got = resolve_system_prompt(Path::new("/tmp"), None).unwrap();
        assert!(got.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn system_prompt_is_caller_only() {
        let lines = vec![Line::user("q")];
        let t = thread_from(&lines, Some("caller system"), None);
        assert_eq!(t.system_prompt, "caller system");
        assert_eq!(t.messages.len(), 1);
    }

    #[test]
    fn user_material_becomes_first_user_message() {
        let lines = vec![Line::user("q")];
        let t = thread_from(&lines, None, Some("## brief\nprepared"));
        assert_eq!(t.messages.len(), 2);
        let first = format!("{:?}", t.messages[0]);
        assert!(first.contains("user"), "{first}");
        assert!(first.contains("## brief\\nprepared"), "{first}");
        let second = format!("{:?}", t.messages[1]);
        assert!(second.contains("user"), "{second}");
        assert!(second.contains("q"), "{second}");
    }

    #[test]
    fn last_assistant_picks_tail() {
        let lines = vec![
            Line::assistant("old"),
            Line::user("q"),
            Line::assistant("new"),
        ];
        assert_eq!(last_assistant(&lines), "new");
    }
}
