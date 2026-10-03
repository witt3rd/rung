//! Drive [`rung_std::agent::run`] with a catalog roster, optional nested
//! [`CatalogSpawn`], and a session file.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rung_std::agent::{self, FailureKind, LoopState, Thread, agentloop};
use rung_std::llm::{ChatMessage, LlmConfig, MessageContent, MessageContentBlock};
use rung_std::tools::{Spawn, TaskRequest, TaskResult, Toolset, WithoutTask};

use serde::{Serialize, Serializer};

use crate::args::{Args, IsolationMode};
use crate::catalog::Kind;
use crate::engine::{Engine, EngineSpec, TurnCtl, TurnDone};
use crate::session::{Line, Session, SessionStore};
use crate::turn_check::{Completion, TurnCheckReport};

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
    /// What memory did this turn. Absent while memory is off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<crate::memory::MemoryReport>,
    /// The answer was forced by the iteration cap ([`agent::AgentResult::forced`]).
    /// ACP reports it; the CLI JSON stays as it was.
    #[serde(skip)]
    pub forced: bool,
    /// Tool results elided after a context overflow this turn
    /// ([`agent::AgentResult::elided`]). ACP reports it; the CLI JSON stays
    /// as it was.
    #[serde(skip)]
    pub elided: usize,
}

/// Why a job gave no outcome. `kind` is set when the agent loop stopped the
/// turn, and says why; it is `None` when the job failed before the loop ran
/// (config, session, arguments). `reason` is the prose the CLI prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobError {
    pub reason: String,
    pub kind: Option<FailureKind>,
}

impl From<String> for JobError {
    fn from(reason: String) -> Self {
        JobError { reason, kind: None }
    }
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
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
    /// The host's `--max-iterations`; `None` leaves the child's toolset default.
    pub max_iterations: Option<u32>,
    pub emitter: Option<Arc<crate::stream::Emitter>>,
    pub extra: JobEx,
    /// The model takes images (`llm.images`); the child loop sends tool images.
    pub tool_images: bool,
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
            self.tool_images,
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
            Err((f, sent)) => {
                sess.status = "error".into();
                record_failure(&mut sess.lines, &f, sent);
                let _ = self.store.save(&sess);
                Err(f.reason)
            }
        }
    }
}

fn drive(
    config: &LlmConfig,
    kind: Kind,
    lines: &[Line],
    max_iterations: Option<u32>,
    emitter: Option<Arc<crate::stream::Emitter>>,
    extra: &JobEx,
    tool_images: bool,
) -> Result<(Line, u32), (agent::Filtered, usize)> {
    let _cancel_guard = crate::mcp::set_session_cancel(extra.cancel.clone());
    let cap = kind.iteration_cap(max_iterations);
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
        tool_images,
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
        Err(f) => Err((f, sent)),
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
            // A turn that stopped replays the steps that ran, never its failure.
            ("assistant", turn) if l.failure.is_some() => {
                messages.extend(turn.iter().flatten().cloned())
            }
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

/// Why a tool's image is missing from a replayed turn.
const HISTORY_IMAGE: &str = "not kept in session history; call the tool again to look";

/// The messages the loop added after the `sent` messages it was given, with
/// large tool results shortened and tool images left as a note (a session
/// file holds no image data).
fn turn_history(transcript: &[ChatMessage], sent: usize) -> Vec<ChatMessage> {
    let mut turn: Vec<ChatMessage> = transcript.iter().skip(sent).cloned().collect();
    for m in &mut turn {
        if let MessageContent::Blocks(blocks) = &mut m.content {
            for b in blocks {
                if let MessageContentBlock::ToolResult {
                    content, images, ..
                } = b
                {
                    if content.chars().count() > HISTORY_TOOL_RESULT_CHARS {
                        let kept: String =
                            content.chars().take(HISTORY_TOOL_RESULT_CHARS).collect();
                        *content = format!("{kept}\n[… shortened in history]");
                    }
                    for img in images.drain(..) {
                        content.push('\n');
                        content.push_str(&img.omitted_note(HISTORY_IMAGE));
                    }
                }
            }
        }
    }
    turn
}

/// The assistant line for a finished turn.
fn turn_line(r: &agent::AgentResult, sent: usize) -> Line {
    Line {
        role: "assistant".into(),
        text: r.final_response.clone(),
        messages: Some(turn_history(&r.transcript, sent)),
        failure: None,
    }
}

/// Record a turn that stopped without an answer: the steps that ran, and
/// why it stopped beside them. An overflow keeps nothing of the turn, its
/// ask included, so the request that overflowed is not sent again.
fn record_failure(lines: &mut Vec<Line>, f: &agent::Filtered, sent: usize) {
    if f.kind == FailureKind::Overflow {
        if lines.last().is_some_and(|l| l.role == "user") {
            lines.pop();
        }
        return;
    }
    lines.push(Line::failed(
        f.reason.clone(),
        turn_history(&f.transcript, sent),
    ));
}

fn last_assistant(lines: &[Line]) -> String {
    lines
        .iter()
        .rev()
        .find(|l| l.role == "assistant")
        .map(|l| l.failure.clone().unwrap_or_else(|| l.text.clone()))
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
    run_job_ex(args, origin, JobEx::default()).map_err(|e| e.reason)
}

pub fn run_job_ex(args: &Args, origin: &Path, extra: JobEx) -> Result<Outcome, JobError> {
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
        crate::background::child_args(args, &id)?;
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
            memory: None,
            forced: false,
            elided: 0,
        });
    }

    let mut sess = store
        .try_load(&id)?
        .unwrap_or_else(|| Session::new(&id, args.kind, &origin));

    if args.prompt.is_none() {
        if store.try_load(&id)?.is_none() {
            return Err(format!("no session {id}").into());
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
            memory: None,
            forced: false,
            elided: 0,
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
            sess.lines.push(Line::failed(e.clone(), Vec::new()));
            let _ = store.save(&sess);
            return Err(e.into());
        }
    };
    // The stream emitter (`--stream`) or, on the ACP prompt path, the
    // client's forwarder of thinking and message deltas.
    let listener: Option<Arc<dyn rung_std::llm::StreamListener>> = match &emitter {
        Some(em) => Some(em.clone() as Arc<dyn rung_std::llm::StreamListener>),
        None => extra.stream_listener.clone(),
    };
    // Reasoning visibility: RUNG_REASONING (e.g. "medium") maps to
    // reasoning_effort / thinking budget. GLM-class models emit
    // reasoning_content deltas regardless; this asks for them.
    crate::engine::reasoning_from_env(&mut config);
    let model = config.model.clone();
    let turn_check = match crate::config::load_turn_check() {
        Ok(t) => t,
        Err(e) => {
            sess.status = "error".into();
            sess.lines.push(Line::failed(e.clone(), Vec::new()));
            let _ = store.save(&sess);
            return Err(e.into());
        }
    };
    let tool_images = match crate::config::load_tool_images() {
        Ok(on) => on,
        Err(e) => {
            sess.status = "error".into();
            sess.lines.push(Line::failed(e.clone(), Vec::new()));
            let _ = store.save(&sess);
            return Err(e.into());
        }
    };

    let memory = match crate::memory::Hooks::load(args.memory.as_ref(), &origin) {
        Ok(m) => m,
        Err(e) => {
            sess.status = "error".into();
            sess.lines.push(Line::failed(e.clone(), Vec::new()));
            let _ = store.save(&sess);
            return Err(e.into());
        }
    };
    let mut memory_report = memory.report();

    let spec = EngineSpec {
        llm: config,
        kind: args.kind,
        scope: crate::engine::resolve_scope(args)?,
        max_iterations: args.max_iterations,
        turn_check,
        tool_images,
        mcp: args.mcp.clone(),
        workspace: origin.clone(),
    };
    // The nested `task` child streams to the same listener as this turn.
    let mut task_config = spec.llm.clone();
    task_config.stream_listener = listener.clone();
    let spawn = CatalogSpawn {
        config: task_config,
        store: store.clone(),
        max_iterations: args.max_iterations,
        emitter: emitter.clone(),
        extra: extra.clone(),
        tool_images,
    };
    let engine = Engine::build(spec)?
        .with_task(Arc::new(spawn))
        .with_outer(memory.toolset(&id))
        .connect()?;

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
    // Recall: shown to this call only, in front of the ask. The session
    // keeps the ask as the user wrote it, so a recall is never replayed.
    let recent: Vec<String> = sess
        .lines
        .iter()
        .filter(|l| l.role == "assistant" && l.failure.is_none() && !l.text.is_empty())
        .map(|l| l.text.clone())
        .collect();
    if let Some((recalled, block)) = memory.recall(args.prompt.as_deref().unwrap_or(""), &recent) {
        if let Some(block) = block {
            crate::memory::inject(&mut thread, &block);
        }
        if let Some(m) = memory_report.as_mut() {
            m.recall = Some(recalled);
        }
    }
    let sent = thread.messages.len();
    let request_text = args.prompt.clone().unwrap_or_default();
    let earlier: Vec<ChatMessage> = sess
        .lines
        .iter()
        .take(sess.lines.len().saturating_sub(1))
        .filter(|l| l.role == "assistant")
        .flat_map(|l| l.messages.clone().unwrap_or_default())
        .collect();
    let ctl = TurnCtl {
        cancel: extra.cancel.clone(),
        stream_listener: listener,
        wrap_tools: tool_wraps(emitter.as_ref(), &extra),
        request: request_text.clone(),
        earlier,
        ..TurnCtl::default()
    };
    match engine.turn(thread, ctl).outcome {
        Ok(TurnDone {
            result: r,
            status,
            turn_check: report,
            api_calls,
            elided,
        }) => {
            sess.lines.push(turn_line(&r, sent));
            // Retain: only a turn that holds a completion becomes memory.
            if let Status::Completed(done) = &status {
                let turn = crate::memory::Turnover::of(
                    done,
                    &request_text,
                    &r.final_response,
                    &id,
                    sess.lines.len() - 1,
                );
                if let Some(kept) = memory.retain(turn)
                    && let Some(m) = memory_report.as_mut()
                {
                    m.retain = Some(kept);
                }
            }
            sess.status = match status {
                Status::Cancelled => "interrupted".into(),
                _ => status.as_str().into(),
            };
            store.save(&sess)?;
            let out = Outcome {
                task_id: id,
                text: r.final_response.clone(),
                status,
                api_calls,
                isolation_path: sess.isolation_path,
                turn_check: report,
                memory: memory_report,
                forced: r.forced,
                elided,
            };
            if let Some(em) = &emitter {
                em.emit_result(&out, &r.usage, &model);
            }
            Ok(out)
        }
        Err(f) if f.kind == FailureKind::Interrupted => {
            sess.status = "interrupted".into();
            record_failure(&mut sess.lines, &f, sent);
            let _ = store.save(&sess);
            Ok(Outcome {
                task_id: id,
                text: String::new(),
                status: Status::Cancelled,
                api_calls: 0,
                isolation_path: sess.isolation_path,
                turn_check: None,
                memory: memory_report,
                forced: false,
                elided: 0,
            })
        }
        Err(f) => {
            sess.status = "error".into();
            record_failure(&mut sess.lines, &f, sent);
            let _ = store.save(&sess);
            if let Some(em) = &emitter {
                em.emit_error(&id, &f.reason, &model);
            }
            Err(JobError {
                reason: f.reason,
                kind: Some(f.kind),
            })
        }
    }
}

/// The turn's tool wrappers, innermost first: the `--stream` observer, then
/// the caller's (ACP tool-call updates).
fn tool_wraps(emitter: Option<&Arc<crate::stream::Emitter>>, extra: &JobEx) -> Vec<WrapTools> {
    let mut wraps: Vec<WrapTools> = Vec::new();
    if let Some(em) = emitter {
        let em = em.clone();
        wraps.push(Arc::new(move |inner| {
            Arc::new(crate::stream::ObservingToolset {
                inner,
                emitter: em.clone(),
            })
        }));
    }
    if let Some(w) = &extra.wrap_tools {
        wraps.push(w.clone());
    }
    wraps
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
                failure: None,
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

    fn failure(kind: FailureKind, transcript: Vec<ChatMessage>) -> agent::Filtered {
        agent::Filtered {
            reason: "it broke".into(),
            kind,
            transcript,
        }
    }

    #[test]
    fn a_failed_turn_replays_its_steps_and_not_its_failure() {
        let ran = vec![
            ChatMessage::user("do it"),
            ChatMessage::assistant("step one"),
        ];
        let mut lines = vec![Line::user("do it")];
        record_failure(&mut lines, &failure(FailureKind::DoomLoop, ran), 1);
        lines.push(Line::user("again"));
        let t = thread_from(&lines, None, None);
        let replay = format!("{:?}", t.messages);
        assert_eq!(t.messages.len(), 3, "{replay}");
        assert!(replay.contains("step one"), "{replay}");
        assert!(!replay.contains("it broke"), "{replay}");
        assert_eq!(lines[1].failure.as_deref(), Some("it broke"));
    }

    #[test]
    fn a_failure_before_any_step_replays_nothing() {
        let lines = vec![
            Line::user("do it"),
            Line::failed("config: no model", Vec::new()),
            Line::user("again"),
        ];
        let t = thread_from(&lines, None, None);
        assert_eq!(t.messages.len(), 2);
        assert!(!format!("{:?}", t.messages).contains("no model"));
    }

    #[test]
    fn an_overflowed_turn_keeps_nothing_of_itself() {
        let mut lines = vec![
            Line::user("first"),
            Line::assistant("answer"),
            Line::user("too much"),
        ];
        let ran = vec![ChatMessage::user("too much"), ChatMessage::assistant("x")];
        record_failure(&mut lines, &failure(FailureKind::Overflow, ran), 1);
        assert_eq!(lines, vec![Line::user("first"), Line::assistant("answer")]);
    }

    #[test]
    fn the_recorded_view_shows_why_a_turn_stopped() {
        let lines = vec![Line::user("q"), Line::failed("auth: bad key", Vec::new())];
        assert_eq!(last_assistant(&lines), "auth: bad key");
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
