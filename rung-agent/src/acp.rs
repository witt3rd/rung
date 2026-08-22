//! ACP v1 agent on stdio via `agent-client-protocol`.
//!
//! Baseline: `initialize`, `session/new`, `session/prompt`, `session/cancel`,
//! `session/update`. Also advertised: load, list, delete, close, set_mode,
//! resume, and unstable `session/fork`. Prompt emits `ToolCall` /
//! `ToolCallUpdate`. Cancel is checked before each LLM call and around each
//! tool. MCP, image, and audio are not claimed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::v1::{
    AgentCapabilities, AuthenticateRequest, AuthenticateResponse, CancelNotification,
    CloseSessionRequest, CloseSessionResponse, ContentBlock, ContentChunk, DeleteSessionRequest,
    DeleteSessionResponse, ForkSessionRequest, ForkSessionResponse, Implementation,
    InitializeRequest, InitializeResponse, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest,
    PromptResponse, ResumeSessionRequest, ResumeSessionResponse, SessionCapabilities,
    SessionCloseCapabilities, SessionDeleteCapabilities, SessionForkCapabilities, SessionId,
    SessionInfo, SessionListCapabilities, SessionMode, SessionModeId, SessionModeState,
    SessionNotification, SessionResumeCapabilities, SessionUpdate, SetSessionModeRequest,
    SetSessionModeResponse, StopReason, TextContent, ToolCall, ToolCallContent, ToolCallStatus,
    ToolCallUpdate, ToolCallUpdateFields, ToolKind,
};
use agent_client_protocol::{
    Agent, Client, ConnectionTo, Error, Responder, Result as AcpResult, Stdio,
};

use crate::args::{Args, IsolationMode};
use crate::catalog::Kind;
use crate::run::{JobEx, run_job_ex};
use crate::session::{Session, SessionStore};
use crate::stream::{NotifyingToolset, ToolNotify};

use serde_json::Value;

#[derive(Clone, Default)]
struct Live {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    /// session_id → kind (cwd lives on the Session file).
    kinds: HashMap<String, Kind>,
    cancelled: HashMap<String, Arc<AtomicBool>>,
}

impl Live {
    fn kind(&self, id: &str) -> Kind {
        self.inner
            .lock()
            .expect("acp state")
            .kinds
            .get(id)
            .copied()
            .unwrap_or(Kind::Implement)
    }

    fn set_kind(&self, id: &str, kind: Kind) {
        self.inner
            .lock()
            .expect("acp state")
            .kinds
            .insert(id.to_string(), kind);
    }

    fn cancel_flag(&self, id: &str) -> Arc<AtomicBool> {
        self.inner
            .lock()
            .expect("acp state")
            .cancelled
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    }

    fn cancel(&self, id: &str) {
        self.cancel_flag(id).store(true, Ordering::SeqCst);
    }

    fn drop_session(&self, id: &str) {
        let mut g = self.inner.lock().expect("acp state");
        g.kinds.remove(id);
        g.cancelled.remove(id);
    }
}

fn modes(current: Kind) -> SessionModeState {
    SessionModeState::new(
        SessionModeId::new(current.as_str()),
        vec![
            SessionMode::new("explore", "Explore").description("Read and search only"),
            SessionMode::new("implement", "Implement").description("Edit, shell, nested task"),
            SessionMode::new("review", "Review").description("Read-only report"),
        ],
    )
}

fn capabilities() -> AgentCapabilities {
    AgentCapabilities::new()
        .load_session(true)
        .session_capabilities(
            SessionCapabilities::new()
                .list(SessionListCapabilities::new())
                .delete(SessionDeleteCapabilities::new())
                .close(SessionCloseCapabilities::new())
                .resume(SessionResumeCapabilities::new())
                .fork(SessionForkCapabilities::new()),
        )
}

fn tool_kind(name: &str) -> ToolKind {
    match name {
        "read_file" | "list_files" | "glob" => ToolKind::Read,
        "grep" => ToolKind::Search,
        "write_file" | "edit" | "apply_patch" => ToolKind::Edit,
        "shell" | "python" => ToolKind::Execute,
        "webfetch" => ToolKind::Fetch,
        "todo" => ToolKind::Think,
        _ => ToolKind::Other,
    }
}

struct AcpNotify {
    connection: ConnectionTo<Client>,
    session_id: SessionId,
}

impl ToolNotify for AcpNotify {
    fn started(&self, id: &str, name: &str, input: &Value) {
        let _ = self.connection.send_notification(SessionNotification::new(
            self.session_id.clone(),
            SessionUpdate::ToolCall(
                ToolCall::new(id.to_string(), name)
                    .kind(tool_kind(name))
                    .status(ToolCallStatus::InProgress)
                    .raw_input(input.clone()),
            ),
        ));
    }

    fn finished(&self, id: &str, _name: &str, result: Result<&str, &str>) {
        let (status, body) = match result {
            Ok(s) => (ToolCallStatus::Completed, s),
            Err(e) => (ToolCallStatus::Failed, e),
        };
        let _ = self.connection.send_notification(SessionNotification::new(
            self.session_id.clone(),
            SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                id.to_string(),
                ToolCallUpdateFields::new()
                    .status(status)
                    .content(vec![ToolCallContent::from(ContentBlock::Text(
                        TextContent::new(body),
                    ))])
                    .raw_output(Value::String(body.to_string())),
            )),
        ));
    }
}

fn abs_cwd(cwd: PathBuf) -> PathBuf {
    if cwd.is_absolute() {
        cwd
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(cwd)
    }
}

fn store_at(cwd: &Path) -> SessionStore {
    SessionStore::in_cwd(cwd)
}

fn sid_str(id: &SessionId) -> String {
    id.to_string()
}

fn prompt_text(blocks: &[ContentBlock]) -> String {
    let mut out = String::new();
    for b in blocks {
        let ContentBlock::Text(t) = b else {
            continue;
        };
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&t.text);
    }
    out
}

fn invalid(msg: impl Into<String>) -> Error {
    Error::invalid_params().data(msg.into())
}

fn send_text(
    connection: &ConnectionTo<Client>,
    session_id: SessionId,
    text: String,
) -> AcpResult<()> {
    if text.is_empty() {
        return Ok(());
    }
    connection.send_notification(SessionNotification::new(
        session_id,
        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(TextContent::new(
            text,
        )))),
    ))
}

fn job_args(process: &Args, id: String, kind: Kind, text: String) -> Args {
    Args {
        task_id: Some(id),
        kind,
        isolation: IsolationMode::None,
        background: false,
        json: false,
        stream: false,
        max_iterations: process.max_iterations,
        system_prompt: process.system_prompt.clone(),
        user_prompt: None,
        tools: process.tools.clone(),
        prompt: Some(text),
        help: false,
        acp: false,
    }
}

/// Speak ACP until stdin closes. CLI `--tools` / `--system-prompt` /
/// `--toolset` / `--max-iterations` apply to each `session/prompt`.
pub fn run(process: Args) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(serve(process)).map_err(|e| e.to_string())
}

async fn serve(process: Args) -> AcpResult<()> {
    let live = Live::default();
    let process = Arc::new(process);

    Agent
        .builder()
        .name("rung-agent")
        .on_receive_request(
            async move |request: InitializeRequest,
                        responder: Responder<InitializeResponse>,
                        _connection: ConnectionTo<Client>| {
                responder.respond(
                    InitializeResponse::new(request.protocol_version)
                        .agent_capabilities(capabilities())
                        .agent_info(
                            Implementation::new("rung-agent", env!("CARGO_PKG_VERSION"))
                                .title("rung-agent"),
                        ),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |_: AuthenticateRequest,
                        responder: Responder<AuthenticateResponse>,
                        _connection: ConnectionTo<Client>| {
                responder.respond(AuthenticateResponse::new())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                let process = process.clone();
                async move |request: NewSessionRequest,
                            responder: Responder<NewSessionResponse>,
                            _connection: ConnectionTo<Client>| {
                    let cwd = abs_cwd(request.cwd);
                    let kind = process.kind;
                    let id = crate::session::new_id();
                    let sess = Session::new(&id, kind, &cwd);
                    store_at(&cwd).save(&sess).map_err(invalid)?;
                    live.set_kind(&id, kind);
                    responder.respond(
                        NewSessionResponse::new(SessionId::new(id.clone())).modes(modes(kind)),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                async move |request: LoadSessionRequest,
                            responder: Responder<LoadSessionResponse>,
                            connection: ConnectionTo<Client>| {
                    let cwd = abs_cwd(request.cwd);
                    let id = sid_str(&request.session_id);
                    let sess = store_at(&cwd).load(&id).map_err(invalid)?;
                    let kind = sess.kind().unwrap_or(Kind::Implement);
                    live.set_kind(&id, kind);
                    if let Some(last) = sess.lines.iter().rev().find(|l| l.role == "assistant") {
                        send_text(&connection, request.session_id.clone(), last.text.clone())?;
                    }
                    responder.respond(LoadSessionResponse::new().modes(modes(kind)))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: ListSessionsRequest,
                        responder: Responder<ListSessionsResponse>,
                        _connection: ConnectionTo<Client>| {
                let cwd = request.cwd.clone().map(abs_cwd).unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                });
                let sessions = store_at(&cwd)
                    .list()
                    .map_err(invalid)?
                    .into_iter()
                    .map(|s| {
                        SessionInfo::new(SessionId::new(s.id.clone()), PathBuf::from(&s.cwd))
                            .title(s.kind)
                    })
                    .collect();
                responder.respond(ListSessionsResponse::new(sessions))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                async move |request: DeleteSessionRequest,
                            responder: Responder<DeleteSessionResponse>,
                            _connection: ConnectionTo<Client>| {
                    let id = sid_str(&request.session_id);
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    store_at(&cwd).delete(&id).map_err(invalid)?;
                    live.drop_session(&id);
                    responder.respond(DeleteSessionResponse::new())
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                async move |request: CloseSessionRequest,
                            responder: Responder<CloseSessionResponse>,
                            _connection: ConnectionTo<Client>| {
                    let id = sid_str(&request.session_id);
                    live.cancel(&id);
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    if let Ok(mut sess) = store_at(&cwd).load(&id) {
                        sess.status = "closed".into();
                        let _ = store_at(&cwd).save(&sess);
                    }
                    responder.respond(CloseSessionResponse::new())
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                async move |request: SetSessionModeRequest,
                            responder: Responder<SetSessionModeResponse>,
                            _connection: ConnectionTo<Client>| {
                    let id = sid_str(&request.session_id);
                    let kind = Kind::parse(request.mode_id.0.as_ref()).map_err(invalid)?;
                    live.set_kind(&id, kind);
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    if let Ok(mut sess) = store_at(&cwd).load(&id) {
                        sess.kind = kind.as_str().into();
                        let _ = store_at(&cwd).save(&sess);
                    }
                    responder.respond(SetSessionModeResponse::new())
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                async move |request: ForkSessionRequest,
                            responder: Responder<ForkSessionResponse>,
                            _connection: ConnectionTo<Client>| {
                    let src = sid_str(&request.session_id);
                    let cwd = abs_cwd(request.cwd);
                    let parent = store_at(&cwd).load(&src).map_err(invalid)?;
                    let id = crate::session::new_id();
                    let kind = parent.kind().unwrap_or(Kind::Implement);
                    let mut child = parent;
                    child.id = id.clone();
                    child.cwd = cwd.to_string_lossy().into_owned();
                    child.status = "new".into();
                    child.pid = Some(std::process::id());
                    store_at(&cwd).save(&child).map_err(invalid)?;
                    live.set_kind(&id, kind);
                    responder
                        .respond(ForkSessionResponse::new(SessionId::new(id)).modes(modes(kind)))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                async move |request: ResumeSessionRequest,
                            responder: Responder<ResumeSessionResponse>,
                            _connection: ConnectionTo<Client>| {
                    let cwd = abs_cwd(request.cwd);
                    let id = sid_str(&request.session_id);
                    let sess = store_at(&cwd).load(&id).map_err(invalid)?;
                    let kind = sess.kind().unwrap_or(Kind::Implement);
                    live.set_kind(&id, kind);
                    responder.respond(ResumeSessionResponse::new().modes(modes(kind)))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                let process = process.clone();
                async move |request: PromptRequest,
                            responder: Responder<PromptResponse>,
                            connection: ConnectionTo<Client>| {
                    let id = sid_str(&request.session_id);
                    let kind = live.kind(&id);
                    let flag = live.cancel_flag(&id);
                    flag.store(false, Ordering::SeqCst);
                    let text = prompt_text(&request.prompt);
                    let session_id = request.session_id.clone();
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    if let Ok(sess) = store_at(&cwd).load(&id)
                        && let Ok(p) = PathBuf::from(&sess.cwd).canonicalize()
                    {
                        let _ = std::env::set_current_dir(p);
                    }
                    let origin = std::env::current_dir().unwrap_or(cwd);
                    if text.is_empty() {
                        return responder.respond(PromptResponse::new(StopReason::EndTurn));
                    }
                    let args = job_args(&process, id.clone(), kind, text);
                    let notify_conn = connection.clone();
                    let notify_sid = session_id.clone();
                    let extra = JobEx {
                        cancel: Some(flag.clone()),
                        wrap_tools: Some(Arc::new(move |inner| {
                            Arc::new(NotifyingToolset::new(
                                inner,
                                AcpNotify {
                                    connection: notify_conn.clone(),
                                    session_id: notify_sid.clone(),
                                },
                            ))
                        })),
                    };
                    let out =
                        tokio::task::spawn_blocking(move || run_job_ex(&args, &origin, extra))
                            .await
                            .map_err(|e| Error::internal_error().data(e.to_string()))?;
                    let cancelled = flag.load(Ordering::SeqCst);
                    match out {
                        Ok(o) => {
                            send_text(&connection, session_id, o.text)?;
                            let reason = if cancelled || o.status == "cancelled" {
                                StopReason::Cancelled
                            } else {
                                StopReason::EndTurn
                            };
                            responder.respond(PromptResponse::new(reason))
                        }
                        Err(e) => responder.respond_with_error(Error::internal_error().data(e)),
                    }
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            {
                let live = live.clone();
                async move |notification: CancelNotification, _connection: ConnectionTo<Client>| {
                    live.cancel(&sid_str(&notification.session_id));
                    Ok(())
                }
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_to(Stdio::new())
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_text_joins_text_blocks() {
        let blocks = vec![
            ContentBlock::Text(TextContent::new("hello")),
            ContentBlock::Text(TextContent::new("world")),
        ];
        assert_eq!(prompt_text(&blocks), "hello\nworld");
    }

    #[test]
    fn modes_include_three_catalogs() {
        let m = modes(Kind::Implement);
        assert_eq!(m.current_mode_id.0.as_ref(), "implement");
        assert_eq!(m.available_modes.len(), 3);
    }

    #[test]
    fn tool_kind_maps_catalog_names() {
        assert_eq!(tool_kind("read_file"), ToolKind::Read);
        assert_eq!(tool_kind("edit"), ToolKind::Edit);
        assert_eq!(tool_kind("shell"), ToolKind::Execute);
        assert_eq!(tool_kind("grep"), ToolKind::Search);
        assert_eq!(tool_kind("webfetch"), ToolKind::Fetch);
        assert_eq!(tool_kind("todo"), ToolKind::Think);
        assert_eq!(tool_kind("task"), ToolKind::Other);
    }

    #[test]
    fn prompt_job_inherits_process_tools_and_system() {
        let process = Args::parse([
            "rung-agent",
            "--acp",
            "--tools",
            "none",
            "--system-prompt",
            "be brief",
            "--max-iterations",
            "3",
            "--toolset",
            "explore",
        ])
        .unwrap();
        let job = job_args(&process, "s1".into(), process.kind, "hello".into());
        assert_eq!(job.tools.as_deref(), Some("none"));
        assert_eq!(job.system_prompt.as_deref(), Some("be brief"));
        assert_eq!(job.max_iterations, Some(3));
        assert_eq!(job.kind, Kind::Explore);
        assert_eq!(job.prompt.as_deref(), Some("hello"));
        assert!(!job.acp);
    }
}
