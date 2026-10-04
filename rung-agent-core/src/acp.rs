//! ACP v1 agent: stdio (`--acp`) or Streamable HTTP (`--acp-http`).
//!
//! Baseline: `initialize`, `session/new`, `session/prompt`, `session/cancel`,
//! `session/update`. Also advertised: load, list, delete, close, set_mode,
//! resume, and unstable `session/fork`. Prompt emits `ToolCall` /
//! `ToolCallUpdate`. Cancel is checked before each LLM call and around each
//! tool. Prompt image/audio/embedded context and MCP HTTP/stdio are claimed.
//!
//! The connection's dispatch loop runs each handler to completion before it
//! reads the next message, so no handler may hold it for a turn. A prompt
//! runs as a spawned task and the loop stays free for `session/cancel`,
//! `session/close` and the rest. Work that must not overlap a turn (the turn
//! itself, and the handlers that write a session file a turn also writes)
//! goes through `queued`: one process-wide FIFO, so turns stay
//! serialized in arrival order as when the loop held them, and the
//! process-global cwd a turn sets is never changed under it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::v1::{
    AgentCapabilities, AuthenticateRequest, AuthenticateResponse, CancelNotification,
    CloseSessionRequest, CloseSessionResponse, ContentBlock, ContentChunk, Cost,
    DeleteSessionRequest, DeleteSessionResponse, EmbeddedResourceResource, ForkSessionRequest,
    ForkSessionResponse, Implementation, InitializeRequest, InitializeResponse,
    ListSessionsRequest, ListSessionsResponse, LoadSessionRequest, LoadSessionResponse,
    McpCapabilities, McpServer, NewSessionRequest, NewSessionResponse, PromptCapabilities,
    PromptRequest, PromptResponse, ResumeSessionRequest, ResumeSessionResponse, Role,
    SessionCapabilities, SessionCloseCapabilities, SessionDeleteCapabilities,
    SessionForkCapabilities, SessionId, SessionInfo, SessionInfoUpdate, SessionListCapabilities,
    SessionMode, SessionModeId, SessionModeState, SessionNotification, SessionResumeCapabilities,
    SessionUpdate, SetSessionModeRequest, SetSessionModeResponse, StopReason, TextContent,
    ToolCall, ToolCallContent, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields, ToolKind,
    UsageUpdate,
};
use agent_client_protocol::{
    Agent, Client, ConnectTo, ConnectionTo, Error, JsonRpcResponse, Responder, Result as AcpResult,
    Stdio,
};

use crate::args::{Args, IsolationMode};
use crate::catalog::Kind;
use crate::mcp::McpSpec;
use crate::run::{JobError, JobEx, MULTIMODAL_PROMPT, Outcome, PromptText, Status, run_job_ex};
use crate::session::{Session, SessionStore};
use crate::stream::{NotifyingToolset, ToolNotify};
use rung_std::agent::FailureKind;
use rung_std::llm::{
    AudioSource, ContentBlockDelta, ImageSource, MessageContentBlock, StreamEvent,
};

use serde_json::Value;
use tokio::sync::oneshot;

#[derive(Clone)]
pub(crate) struct Live {
    inner: Arc<Mutex<Inner>>,
    /// The process cwd at start. A turn moves the process cwd, so handlers
    /// that run beside one resolve against this instead.
    launch: Arc<PathBuf>,
}

#[derive(Default)]
struct Inner {
    /// session_id → kind (cwd lives on the Session file).
    kinds: HashMap<String, Kind>,
    /// session_id → the flag the next prompt takes. A cancel sets it and
    /// removes it, so every turn that took it (running or queued) stops and
    /// a later prompt starts clean.
    cancelled: HashMap<String, Arc<AtomicBool>>,
    mcp: HashMap<String, Vec<McpSpec>>,
    /// session_id → per-session system text from `session/new` `_meta`.
    system: HashMap<String, String>,
    /// session_id → absolute cwd whose store holds the session file.
    cwds: HashMap<String, PathBuf>,
    /// Released when the last enqueued work ends; the next waits on it.
    tail: Option<oneshot::Receiver<()>>,
}

/// A place in [`Live`]'s queue. [`Slot::ready`] waits for the work before
/// it; dropping the slot lets the next one go, on every path out.
struct Slot {
    before: Option<oneshot::Receiver<()>>,
    _done: oneshot::Sender<()>,
}

impl Slot {
    async fn ready(mut self) -> Self {
        if let Some(before) = self.before.take() {
            // Err means the sender dropped: the work before is over.
            let _ = before.await;
        }
        self
    }
}

impl Live {
    pub(crate) fn new() -> Self {
        Live {
            inner: Arc::default(),
            launch: Arc::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        }
    }

    /// Take the next place in the process-wide queue. Synchronous, so
    /// places follow message arrival order.
    fn enqueue(&self) -> Slot {
        let (done, after) = oneshot::channel();
        let before = self.inner.lock().expect("acp state").tail.replace(after);
        Slot {
            before,
            _done: done,
        }
    }

    /// `cwd` made absolute against the launch cwd.
    fn abs(&self, cwd: PathBuf) -> PathBuf {
        if cwd.is_absolute() {
            cwd
        } else {
            self.launch.join(cwd)
        }
    }

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

    /// The flag a prompt arriving now takes; set by the next cancel.
    fn cancel_flag(&self, id: &str) -> Arc<AtomicBool> {
        self.inner
            .lock()
            .expect("acp state")
            .cancelled
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    }

    /// Stop every turn of `id` that has arrived. One that arrives later
    /// takes a fresh flag.
    fn cancel(&self, id: &str) {
        let flag = self.inner.lock().expect("acp state").cancelled.remove(id);
        if let Some(flag) = flag {
            flag.store(true, Ordering::SeqCst);
        }
    }

    fn drop_session(&self, id: &str) {
        let mut g = self.inner.lock().expect("acp state");
        g.kinds.remove(id);
        g.cancelled.remove(id);
        g.mcp.remove(id);
        g.system.remove(id);
        g.cwds.remove(id);
    }

    fn set_cwd(&self, id: &str, cwd: &Path) {
        self.inner
            .lock()
            .expect("acp state")
            .cwds
            .insert(id.to_string(), cwd.to_path_buf());
    }

    /// The session's store: its remembered cwd, else the launch cwd.
    fn store(&self, id: &str) -> SessionStore {
        let cwd = self
            .inner
            .lock()
            .expect("acp state")
            .cwds
            .get(id)
            .cloned()
            .unwrap_or_else(|| self.launch.to_path_buf());
        store_at(&cwd)
    }

    fn set_system(&self, id: &str, text: Option<String>) {
        let mut g = self.inner.lock().expect("acp state");
        match text {
            Some(t) if !t.trim().is_empty() => {
                g.system.insert(id.to_string(), t);
            }
            _ => {
                g.system.remove(id);
            }
        }
    }

    fn system(&self, id: &str) -> Option<String> {
        self.inner
            .lock()
            .expect("acp state")
            .system
            .get(id)
            .cloned()
    }

    fn set_mcp(&self, id: &str, specs: Vec<McpSpec>) {
        self.inner
            .lock()
            .expect("acp state")
            .mcp
            .insert(id.to_string(), specs);
    }

    fn mcp(&self, id: &str) -> Vec<McpSpec> {
        self.inner
            .lock()
            .expect("acp state")
            .mcp
            .get(id)
            .cloned()
            .unwrap_or_default()
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
        .prompt_capabilities(
            PromptCapabilities::new()
                .image(true)
                .audio(true)
                .embedded_context(true),
        )
        .mcp_capabilities(McpCapabilities::new().http(true))
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

/// Answer `responder` with `work`, run off the dispatch loop once all work
/// enqueued before it has ended. The spawned task never fails: an error
/// would shut the connection down, so it goes to the client instead.
fn queued<T: JsonRpcResponse>(
    live: &Live,
    connection: &ConnectionTo<Client>,
    responder: Responder<T>,
    work: impl Future<Output = AcpResult<T>> + Send + 'static,
) -> AcpResult<()> {
    let slot = live.enqueue();
    connection.spawn(async move {
        let _slot = slot.ready().await;
        let _ = responder.respond_with_result(work.await);
        Ok(())
    })
}

fn store_at(cwd: &Path) -> SessionStore {
    SessionStore::in_cwd(cwd)
}

fn sid_str(id: &SessionId) -> String {
    id.to_string()
}

/// A block is context, not the ask, when its ACP `annotations.audience` is
/// non-empty and names only the assistant.
fn is_context(b: &ContentBlock) -> bool {
    let ann = match b {
        ContentBlock::Text(t) => t.annotations.as_ref(),
        ContentBlock::Image(i) => i.annotations.as_ref(),
        ContentBlock::Audio(a) => a.annotations.as_ref(),
        ContentBlock::ResourceLink(r) => r.annotations.as_ref(),
        ContentBlock::Resource(e) => e.annotations.as_ref(),
        _ => None,
    };
    ann.and_then(|a| a.audience.as_ref())
        .is_some_and(|aud| !aud.is_empty() && aud.iter().all(|r| matches!(r, Role::Assistant)))
}

/// One ACP prompt, split for the turn.
struct Prompt {
    /// Every block's text, joined: the job text.
    text: String,
    /// Every block, as the model sees it.
    blocks: Vec<MessageContentBlock>,
    /// The text of the blocks not marked as context (all text when every
    /// block is marked or none is).
    ask: String,
    /// Each text block and its mark, only when some but not every block is
    /// marked.
    texts: Option<Vec<PromptText>>,
}

fn prompt_parts(blocks: &[ContentBlock]) -> Prompt {
    let (text, out) = prompt_parts_all(blocks);
    let unmarked: Vec<ContentBlock> = blocks.iter().filter(|b| !is_context(b)).cloned().collect();
    if unmarked.is_empty() || unmarked.len() == blocks.len() {
        return Prompt {
            ask: text.clone(),
            text,
            blocks: out,
            texts: None,
        };
    }
    let ask = prompt_parts_all(&unmarked).0;
    let texts = blocks
        .iter()
        .filter_map(|b| {
            let (text, out) = prompt_parts_all(std::slice::from_ref(b));
            matches!(out.first(), Some(MessageContentBlock::Text { .. })).then(|| PromptText {
                text,
                context: is_context(b),
            })
        })
        .collect();
    Prompt {
        text,
        blocks: out,
        ask,
        texts: Some(texts),
    }
}

fn prompt_parts_all(blocks: &[ContentBlock]) -> (String, Vec<MessageContentBlock>) {
    let mut text = String::new();
    let mut out = Vec::new();
    for b in blocks {
        match b {
            ContentBlock::Text(t) => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&t.text);
                out.push(MessageContentBlock::Text {
                    text: t.text.clone(),
                    cache: None,
                });
            }
            ContentBlock::Image(img) => {
                out.push(MessageContentBlock::Image {
                    source: ImageSource {
                        source_type: "base64".into(),
                        media_type: img.mime_type.clone(),
                        data: img.data.clone(),
                    },
                    cache: None,
                });
            }
            ContentBlock::Audio(a) => {
                out.push(MessageContentBlock::Audio {
                    source: AudioSource {
                        media_type: a.mime_type.clone(),
                        data: a.data.clone(),
                    },
                    cache: None,
                });
            }
            ContentBlock::ResourceLink(r) => {
                let line = match &r.description {
                    Some(d) => format!("[resource {}]({}) {}", r.name, r.uri, d),
                    None => format!("[resource {}]({})", r.name, r.uri),
                };
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&line);
                out.push(MessageContentBlock::Text {
                    text: line,
                    cache: None,
                });
            }
            ContentBlock::Resource(e) => match &e.resource {
                EmbeddedResourceResource::TextResourceContents(t) => {
                    let line = format!("[resource {}]\n{}", t.uri, t.text);
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&line);
                    out.push(MessageContentBlock::Text {
                        text: line,
                        cache: None,
                    });
                }
                EmbeddedResourceResource::BlobResourceContents(b) => {
                    let mime = b
                        .mime_type
                        .clone()
                        .unwrap_or_else(|| "application/octet-stream".into());
                    if mime.starts_with("image/") {
                        out.push(MessageContentBlock::Image {
                            source: ImageSource {
                                source_type: "base64".into(),
                                media_type: mime,
                                data: b.blob.clone(),
                            },
                            cache: None,
                        });
                    } else if mime.starts_with("audio/") {
                        out.push(MessageContentBlock::Audio {
                            source: AudioSource {
                                media_type: mime,
                                data: b.blob.clone(),
                            },
                            cache: None,
                        });
                    } else {
                        let line = format!("[blob {} {}]", b.uri, mime);
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(&line);
                        out.push(MessageContentBlock::Text {
                            text: line,
                            cache: None,
                        });
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    (text, out)
}

fn mcp_from_acp(servers: &[McpServer]) -> Vec<McpSpec> {
    let mut out = Vec::new();
    for s in servers {
        match s {
            McpServer::Http(h) => out.push(McpSpec::Http {
                name: h.name.clone(),
                url: h.url.clone(),
                headers: h
                    .headers
                    .iter()
                    .map(|h| (h.name.clone(), h.value.clone()))
                    .collect(),
            }),
            McpServer::Sse(h) => out.push(McpSpec::Http {
                name: h.name.clone(),
                url: h.url.clone(),
                headers: h
                    .headers
                    .iter()
                    .map(|h| (h.name.clone(), h.value.clone()))
                    .collect(),
            }),
            McpServer::Stdio(st) => out.push(McpSpec::Stdio {
                name: st.name.clone(),
                command: st.command.clone(),
                args: st.args.clone(),
                env: st
                    .env
                    .iter()
                    .map(|e| (e.name.clone(), e.value.clone()))
                    .collect(),
            }),
            _ => {}
        }
    }
    out
}

fn invalid(msg: impl Into<String>) -> Error {
    Error::invalid_params().data(msg.into())
}

/// Forward model deltas as ACP updates while the provider response is open.
struct ThoughtForwarder {
    connection: ConnectionTo<Client>,
    session_id: SessionId,
    streamed_text: Arc<AtomicBool>,
    /// `used` of the last `usage_update` sent this turn.
    last_used: Arc<AtomicU64>,
}

impl rung_std::llm::StreamListener for ThoughtForwarder {
    fn on_event(&self, event: StreamEvent) {
        let update = update_for_event(event, &self.streamed_text);
        if let SessionUpdate::UsageUpdate(u) = &update {
            self.last_used.store(u.used, Ordering::SeqCst);
        }
        let _ = self
            .connection
            .send_notification(SessionNotification::new(self.session_id.clone(), update));
    }
}

fn raw_meta(event: &StreamEvent) -> serde_json::Map<String, Value> {
    let mut meta = serde_json::Map::new();
    meta.insert(
        "rung".into(),
        serde_json::to_value(event).unwrap_or(Value::Null),
    );
    meta
}

fn update_for_event(event: StreamEvent, streamed_text: &AtomicBool) -> SessionUpdate {
    let meta = raw_meta(&event);
    match event {
        StreamEvent::ContentBlockDelta {
            delta: ContentBlockDelta::ThinkingDelta(text),
            ..
        } => SessionUpdate::AgentThoughtChunk(
            ContentChunk::new(ContentBlock::Text(TextContent::new(text))).meta(meta),
        ),
        StreamEvent::ContentBlockDelta {
            delta: ContentBlockDelta::TextDelta(text),
            ..
        } => {
            streamed_text.store(true, Ordering::SeqCst);
            SessionUpdate::AgentMessageChunk(
                ContentChunk::new(ContentBlock::Text(TextContent::new(text))).meta(meta),
            )
        }
        StreamEvent::MessageDelta {
            usage: Some(usage), ..
        } => {
            let used = u64::from(usage.input_tokens) + u64::from(usage.output_tokens);
            let mut update = UsageUpdate::new(used, 0).meta(meta);
            if let Some(cost) = usage.cost_usd {
                update = update.cost(Cost::new(cost, "USD"));
            }
            SessionUpdate::UsageUpdate(update)
        }
        _ => SessionUpdate::SessionInfoUpdate(SessionInfoUpdate::new().meta(meta)),
    }
}

fn send_text_if_unstreamed(
    connection: &ConnectionTo<Client>,
    session_id: SessionId,
    text: String,
    streamed_text: &AtomicBool,
) -> AcpResult<()> {
    if streamed_text.load(Ordering::SeqCst) {
        Ok(())
    } else {
        send_text(connection, session_id, text)
    }
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

/// The number after `needle` in `text` (`128,000` reads as 128000).
fn number_after(text: &str, needle: &str) -> Option<u64> {
    let rest = &text[text.find(needle)? + needle.len()..];
    let digits: String = rest
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .filter(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// The token figures a provider gives when it refuses a request as over its
/// context window: what the request held, and the window. OpenAI, OpenRouter
/// and vLLM say "maximum context length is M tokens. However, your messages
/// resulted in (you requested) N tokens"; Anthropic says "prompt is too long:
/// N tokens > M maximum".
fn stated_window(reason: &str) -> (Option<u64>, Option<u64>) {
    let m = reason.to_ascii_lowercase();
    let used = [
        "resulted in",
        "requested about",
        "you requested",
        "prompt is too long:",
    ]
    .iter()
    .find_map(|n| number_after(&m, n));
    let size = [
        "maximum context length is",
        "context length of",
        "context window of",
        "tokens >",
    ]
    .iter()
    .find_map(|n| number_after(&m, n));
    (used, size)
}

/// The `usage_update` sent before a typed overflow. `used` and `size` are
/// the provider's figures when its refusal states them. Otherwise `used` is
/// that of the turn's last `usage_update` and `size` is 0 (unknown), as on
/// every other update. `_meta.rung.overflow` says which figures the provider
/// stated (`null` where it did not).
fn overflow_usage(reason: &str, last_used: u64) -> SessionUpdate {
    let (used, size) = stated_window(reason);
    let mut meta = serde_json::Map::new();
    meta.insert(
        "rung".into(),
        serde_json::json!({"overflow": {"used": used, "size": size}}),
    );
    SessionUpdate::UsageUpdate(
        UsageUpdate::new(used.unwrap_or(last_used), size.unwrap_or(0)).meta(meta),
    )
}

/// How a turn ended when it did not end plainly. It rides as
/// `_meta.rung.terminal` on a prompt result and as `data.rung.terminal` on a
/// prompt error: `{"state", "reason"}`, plus `"kind"` for `failed`. A plain
/// end (`end_turn`, `cancelled`, `max_tokens`) carries none, so those
/// responses are as before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Terminal {
    /// The cap withdrew the tools on the last call and the model answered.
    CapForced,
    /// The cap (or budget) ran out with no answer.
    CapExhausted,
    Refused,
    Overflow,
    DoomLoop,
    /// Any other unrecoverable failure; the kind names it.
    Failed(&'static str),
}

impl Terminal {
    /// The terminal for a loop failure. `None` for an interrupt: that is
    /// `cancelled`, a plain end.
    fn of(kind: FailureKind) -> Option<Terminal> {
        Some(match kind {
            FailureKind::Interrupted => return None,
            FailureKind::MaxIterations | FailureKind::BudgetExhausted => Terminal::CapExhausted,
            FailureKind::Refusal => Terminal::Refused,
            FailureKind::Overflow => Terminal::Overflow,
            FailureKind::DoomLoop => Terminal::DoomLoop,
            FailureKind::ContentPolicy => Terminal::Failed("content_policy"),
            FailureKind::Auth => Terminal::Failed("auth"),
            FailureKind::Forbidden => Terminal::Failed("forbidden"),
            FailureKind::Quota => Terminal::Failed("quota"),
            FailureKind::Config => Terminal::Failed("config"),
            FailureKind::Provider => Terminal::Failed("provider"),
        })
    }

    fn state(self) -> &'static str {
        match self {
            Terminal::CapForced => "cap_forced",
            Terminal::CapExhausted => "cap_exhausted",
            Terminal::Refused => "refused",
            Terminal::Overflow => "overflow",
            Terminal::DoomLoop => "doom_loop",
            Terminal::Failed(_) => "failed",
        }
    }

    /// The ACP stop reason, where ACP has one. The rest are errors.
    fn stop_reason(self) -> Option<StopReason> {
        match self {
            Terminal::CapForced | Terminal::CapExhausted => Some(StopReason::MaxTurnRequests),
            Terminal::Refused => Some(StopReason::Refusal),
            Terminal::Overflow | Terminal::DoomLoop | Terminal::Failed(_) => None,
        }
    }

    fn json(self, reason: &str) -> Value {
        let mut t = serde_json::json!({"state": self.state(), "reason": reason});
        if let Terminal::Failed(kind) = self {
            t["kind"] = kind.into();
        }
        t
    }
}

/// `_meta.rung` for a prompt response: the status and the turn check's
/// reading while the check is on, what memory did while it is not off,
/// `elided` when a context overflow had the
/// turn elide its oldest tool results, and the terminal when the turn did not
/// end plainly. `None` when there is none of these, so the response is as
/// before.
fn prompt_meta(
    o: Option<&Outcome>,
    terminal: Option<(Terminal, &str)>,
) -> Option<serde_json::Map<String, Value>> {
    let mut rung = serde_json::Map::new();
    if let Some(o) = o
        && let Some(tc) = o.turn_check.as_ref()
    {
        rung.insert("status".into(), o.status.as_str().into());
        rung.insert("turn_check".into(), serde_json::json!(tc));
    }
    if let Some(o) = o
        && o.elided > 0
    {
        rung.insert(
            "elided".into(),
            serde_json::json!({"tool_results": o.elided}),
        );
    }
    if let Some(o) = o
        && let Some(m) = o.memory.as_ref()
    {
        rung.insert("memory".into(), serde_json::json!(m));
    }
    if let Some((t, reason)) = terminal {
        rung.insert("terminal".into(), t.json(reason));
    }
    if rung.is_empty() {
        return None;
    }
    let mut meta = serde_json::Map::new();
    meta.insert("rung".into(), Value::Object(rung));
    Some(meta)
}

/// The response to a prompt whose job returned no outcome. A loop failure
/// ACP has a stop reason for is a result; the rest are -32603 with the
/// terminal as `data.rung.terminal`. A failure before the loop ran (config,
/// session) is not a turn's end and stays a -32603 with prose `data`.
fn prompt_failure(e: JobError, cancelled: bool) -> Result<PromptResponse, Error> {
    if cancelled || e.kind == Some(FailureKind::Interrupted) {
        return Ok(PromptResponse::new(StopReason::Cancelled));
    }
    let Some(t) = e.kind.and_then(Terminal::of) else {
        return Err(Error::internal_error().data(e.reason));
    };
    match t.stop_reason() {
        Some(stop) => Ok(PromptResponse::new(stop).meta(prompt_meta(None, Some((t, &e.reason))))),
        None => Err(Error::internal_error()
            .data(serde_json::json!({"rung": {"terminal": t.json(&e.reason)}}))),
    }
}

/// The session's own system text: `_meta.systemPrompt` on `session/new`.
fn session_system(meta: Option<&serde_json::Map<String, Value>>) -> Option<String> {
    meta?.get("systemPrompt")?.as_str().map(str::to_string)
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
        acp_http: None,
        acp_token: None,
        mcp: Vec::new(),
        memory: process.memory.clone(),
    }
}

/// Speak ACP until stdin closes, or until the HTTP listener stops.
/// CLI `--tools` / `--system-prompt` / `--toolset` / `--max-iterations`
/// apply to each `session/prompt`.
pub fn run(process: Args) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async {
        if let Some(addr) = process.acp_http.clone() {
            crate::acp_http::listen(process, addr).await
        } else {
            let live = Live::new();
            connect_agent(Arc::new(process), live, Stdio::new())
                .await
                .map_err(|e| e.to_string())
        }
    })
}

pub(crate) async fn connect_agent(
    process: Arc<Args>,
    live: Live,
    transport: impl ConnectTo<Agent> + 'static,
) -> AcpResult<()> {
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
                    let cwd = live.abs(request.cwd);
                    let kind = process.kind;
                    let id = crate::session::new_id();
                    let sess = Session::new(&id, kind, &cwd);
                    store_at(&cwd).save(&sess).map_err(invalid)?;
                    live.set_cwd(&id, &cwd);
                    live.set_kind(&id, kind);
                    live.set_mcp(&id, mcp_from_acp(&request.mcp_servers));
                    live.set_system(&id, session_system(request.meta.as_ref()));
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
                    let cwd = live.abs(request.cwd);
                    let id = sid_str(&request.session_id);
                    let sess = store_at(&cwd).load(&id).map_err(invalid)?;
                    let kind = sess.kind().unwrap_or(Kind::Implement);
                    live.set_cwd(&id, &cwd);
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
            {
                let live = live.clone();
                async move |request: ListSessionsRequest,
                            responder: Responder<ListSessionsResponse>,
                            _connection: ConnectionTo<Client>| {
                    let cwd = request
                        .cwd
                        .clone()
                        .map(|c| live.abs(c))
                        .unwrap_or_else(|| live.launch.to_path_buf());
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
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                async move |request: DeleteSessionRequest,
                            responder: Responder<DeleteSessionResponse>,
                            connection: ConnectionTo<Client>| {
                    // Stop the session's turns now; delete after they end, so
                    // a turn's last write cannot bring the file back.
                    let id = sid_str(&request.session_id);
                    live.cancel(&id);
                    let store = live.store(&id);
                    let after = live.clone();
                    queued(&live, &connection, responder, async move {
                        store.delete(&id).map_err(invalid)?;
                        after.drop_session(&id);
                        Ok(DeleteSessionResponse::new())
                    })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                async move |request: CloseSessionRequest,
                            responder: Responder<CloseSessionResponse>,
                            connection: ConnectionTo<Client>| {
                    // Stop the session's turns now; mark it closed after they
                    // end, so a turn's last write does not undo it.
                    let id = sid_str(&request.session_id);
                    live.cancel(&id);
                    let store = live.store(&id);
                    queued(&live, &connection, responder, async move {
                        if let Ok(mut sess) = store.load(&id) {
                            sess.status = "closed".into();
                            let _ = store.save(&sess);
                        }
                        Ok(CloseSessionResponse::new())
                    })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                async move |request: SetSessionModeRequest,
                            responder: Responder<SetSessionModeResponse>,
                            connection: ConnectionTo<Client>| {
                    // The next prompt takes the mode now; the file records it
                    // after the turns before have written theirs.
                    let id = sid_str(&request.session_id);
                    let kind = Kind::parse(request.mode_id.0.as_ref()).map_err(invalid)?;
                    live.set_kind(&id, kind);
                    let store = live.store(&id);
                    queued(&live, &connection, responder, async move {
                        if let Ok(mut sess) = store.load(&id) {
                            sess.kind = kind.as_str().into();
                            let _ = store.save(&sess);
                        }
                        Ok(SetSessionModeResponse::new())
                    })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let live = live.clone();
                async move |request: ForkSessionRequest,
                            responder: Responder<ForkSessionResponse>,
                            connection: ConnectionTo<Client>| {
                    // Fork a settled parent, not one a turn is writing.
                    let src = sid_str(&request.session_id);
                    let cwd = live.abs(request.cwd);
                    let after = live.clone();
                    queued(&live, &connection, responder, async move {
                        let live = after;
                        let parent = store_at(&cwd).load(&src).map_err(invalid)?;
                        let id = crate::session::new_id();
                        let kind = parent.kind().unwrap_or(Kind::Implement);
                        let mut child = parent;
                        child.id = id.clone();
                        child.cwd = cwd.to_string_lossy().into_owned();
                        child.status = "new".into();
                        child.pid = Some(std::process::id());
                        store_at(&cwd).save(&child).map_err(invalid)?;
                        live.set_cwd(&id, &cwd);
                        live.set_kind(&id, kind);
                        live.set_mcp(&id, live.mcp(&src));
                        Ok(ForkSessionResponse::new(SessionId::new(id)).modes(modes(kind)))
                    })
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
                    let cwd = live.abs(request.cwd);
                    let id = sid_str(&request.session_id);
                    let sess = store_at(&cwd).load(&id).map_err(invalid)?;
                    let kind = sess.kind().unwrap_or(Kind::Implement);
                    live.set_cwd(&id, &cwd);
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
                    // Everything the turn reads from `Live` is taken here, in
                    // arrival order; the turn itself runs off the loop, after
                    // the work queued before it.
                    let id = sid_str(&request.session_id);
                    let kind = live.kind(&id);
                    let flag = live.cancel_flag(&id);
                    let Prompt {
                        mut text,
                        blocks,
                        ask,
                        texts,
                    } = prompt_parts(&request.prompt);
                    if blocks.is_empty() {
                        return responder.respond(PromptResponse::new(StopReason::EndTurn));
                    }
                    if text.is_empty() {
                        text = MULTIMODAL_PROMPT.into();
                    }
                    let session_id = request.session_id.clone();
                    let store = live.store(&id);
                    let launch = live.launch.clone();
                    let mut args = job_args(&process, id.clone(), kind, text);
                    args.mcp = live.mcp(&id);
                    let notify_conn = connection.clone();
                    let notify_sid = session_id.clone();
                    let streamed_text = Arc::new(AtomicBool::new(false));
                    let last_used = Arc::new(AtomicU64::new(0));
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
                        stream_listener: Some(Arc::new(ThoughtForwarder {
                            connection: connection.clone(),
                            session_id: session_id.clone(),
                            streamed_text: streamed_text.clone(),
                            last_used: last_used.clone(),
                        })),
                        prompt_blocks: Some(blocks),
                        ask_text: Some(ask),
                        prompt_text: texts,
                        system_append: live.system(&id),
                    };
                    let turn_conn = connection.clone();
                    let turn_process = process.clone();
                    queued(&live, &connection, responder, async move {
                        let connection = turn_conn;
                        let out = tokio::task::spawn_blocking(move || {
                            // The process cwd is the turn's while it runs;
                            // the queue keeps every other turn out.
                            let cwd = store
                                .load(&id)
                                .ok()
                                .and_then(|s| PathBuf::from(&s.cwd).canonicalize().ok())
                                .unwrap_or_else(|| launch.to_path_buf());
                            let _ = std::env::set_current_dir(&cwd);
                            let origin = std::env::current_dir().unwrap_or(cwd);
                            run_job_ex(&args, &origin, extra)
                        })
                        .await
                        .map_err(|e| Error::internal_error().data(e.to_string()))?;
                        let cancelled = flag.load(Ordering::SeqCst);
                        match out {
                            Ok(o) => {
                                let plain = cancelled
                                    || o.status == Status::Cancelled
                                    || o.status == Status::Truncated;
                                let forced = (o.forced && !plain).then(|| {
                                    format!(
                                        "iteration cap ({}) reached; the last call had no tools",
                                        kind.iteration_cap(turn_process.max_iterations)
                                    )
                                });
                                let meta = prompt_meta(
                                    Some(&o),
                                    forced.as_deref().map(|r| (Terminal::CapForced, r)),
                                );
                                send_text_if_unstreamed(
                                    &connection,
                                    session_id,
                                    o.text,
                                    &streamed_text,
                                )?;
                                let reason = if cancelled || o.status == Status::Cancelled {
                                    StopReason::Cancelled
                                } else if o.status == Status::Truncated {
                                    StopReason::MaxTokens
                                } else if forced.is_some() {
                                    StopReason::MaxTurnRequests
                                } else {
                                    // ACP has no "unverified": the turn ended, and
                                    // `_meta.rung` says what the check made of it.
                                    StopReason::EndTurn
                                };
                                let mut response = PromptResponse::new(reason);
                                if let Some(meta) = meta {
                                    response = response.meta(meta);
                                }
                                Ok(response)
                            }
                            Err(e) => {
                                if !cancelled && e.kind == Some(FailureKind::Overflow) {
                                    let usage =
                                        overflow_usage(&e.reason, last_used.load(Ordering::SeqCst));
                                    connection.send_notification(SessionNotification::new(
                                        session_id, usage,
                                    ))?;
                                }
                                prompt_failure(e, cancelled)
                            }
                        }
                    })
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
        .connect_to(transport)
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
        let p = prompt_parts(&blocks);
        assert_eq!(p.text, "hello\nworld");
        assert_eq!(p.blocks.len(), 2);
        assert_eq!(p.texts, None, "nothing marked");
    }

    #[test]
    fn prompt_parts_keep_image() {
        let blocks = vec![ContentBlock::Image(
            agent_client_protocol::schema::v1::ImageContent::new("QQ==", "image/png"),
        )];
        let p = prompt_parts(&blocks);
        assert!(p.text.is_empty());
        assert!(matches!(p.blocks[0], MessageContentBlock::Image { .. }));
    }

    fn marked(text: &str) -> ContentBlock {
        ContentBlock::Text(TextContent::new(text).annotations(
            agent_client_protocol::schema::v1::Annotations::new().audience(vec![Role::Assistant]),
        ))
    }

    /// The text blocks rejoin to the job text byte for byte, empty blocks and
    /// resources included, so a line that leaves nothing out is the job text.
    #[test]
    fn the_text_blocks_rejoin_to_the_job_text() {
        let blocks = vec![
            ContentBlock::Text(TextContent::new("")),
            marked("orientation\nline two"),
            ContentBlock::Image(agent_client_protocol::schema::v1::ImageContent::new(
                "QQ==",
                "image/png",
            )),
            ContentBlock::Text(TextContent::new("")),
            ContentBlock::ResourceLink(agent_client_protocol::schema::v1::ResourceLink::new(
                "spec",
                "file:///spec.md",
            )),
            ContentBlock::Text(TextContent::new("the ask")),
        ];
        let p = prompt_parts(&blocks);
        let texts = p.texts.expect("some blocks marked");
        assert_eq!(texts.len(), 5, "the image has no text");
        assert_eq!(
            texts.iter().map(|t| t.context).collect::<Vec<_>>(),
            [false, true, false, false, false]
        );
        assert_eq!(
            crate::run::join_text(texts.iter().map(|t| t.text.as_str())),
            p.text
        );
        assert_eq!(p.ask, "[resource spec](file:///spec.md)\nthe ask");
    }

    #[test]
    fn every_block_marked_carries_no_texts() {
        let p = prompt_parts(&[marked("a"), marked("b")]);
        assert_eq!(p.ask, "a\nb");
        assert_eq!(p.texts, None);
    }

    #[test]
    fn text_delta_becomes_an_immediate_acp_message_chunk() {
        let streamed = AtomicBool::new(false);
        let update = update_for_event(
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentBlockDelta::TextDelta("Hel".into()),
            },
            &streamed,
        );
        assert!(matches!(update, SessionUpdate::AgentMessageChunk(_)));
        assert!(streamed.load(Ordering::SeqCst));
    }

    #[test]
    fn every_rung_stream_event_has_an_acp_update() {
        let streamed = AtomicBool::new(false);
        let update = update_for_event(StreamEvent::MessageStop, &streamed);
        let SessionUpdate::SessionInfoUpdate(info) = update else {
            panic!("message stop was dropped");
        };
        assert!(info.meta.unwrap().contains_key("rung"));
    }

    #[test]
    fn session_system_reads_meta_system_prompt() {
        let meta: serde_json::Map<String, Value> =
            serde_json::from_str(r#"{"systemPrompt":"you are in a project channel"}"#).unwrap();
        assert_eq!(
            session_system(Some(&meta)).as_deref(),
            Some("you are in a project channel")
        );
        assert_eq!(session_system(None), None);
        let live = Live::new();
        live.set_system("s1", session_system(Some(&meta)));
        assert_eq!(
            live.system("s1").as_deref(),
            Some("you are in a project channel")
        );
        live.drop_session("s1");
        assert_eq!(live.system("s1"), None);
    }

    #[test]
    fn an_overflow_reports_the_figures_the_provider_stated() {
        let openai = "invalid-request (context-overflow): This model's maximum context length is 128,000 tokens. However, your messages resulted in 130,512 tokens.";
        assert_eq!(stated_window(openai), (Some(130_512), Some(128_000)));
        let openrouter = "This endpoint's maximum context length is 131072 tokens. However, you requested about 140000 tokens (139000 of text input).";
        assert_eq!(stated_window(openrouter), (Some(140_000), Some(131_072)));
        let anthropic = "invalid-request (context-overflow): prompt is too long: 210000 tokens > 200000 maximum";
        assert_eq!(stated_window(anthropic), (Some(210_000), Some(200_000)));
        assert_eq!(stated_window("request_too_large"), (None, None));

        let SessionUpdate::UsageUpdate(u) = overflow_usage(openai, 7) else {
            panic!("not a usage_update");
        };
        assert_eq!((u.used, u.size), (130_512, 128_000));
        // Unstated: the turn's last measured context, and an unknown window.
        let SessionUpdate::UsageUpdate(u) = overflow_usage("prompt too long", 7) else {
            panic!("not a usage_update");
        };
        assert_eq!((u.used, u.size), (7, 0));
        let meta = serde_json::to_value(u.meta).unwrap();
        assert_eq!(
            meta,
            serde_json::json!({"rung": {"overflow": {"used": null, "size": null}}})
        );
    }

    #[test]
    fn usage_update_keeps_the_complete_rung_measurement_in_meta() {
        let streamed = AtomicBool::new(false);
        let mut usage = rung_std::llm::Usage::from_openai(100, 20, 80, 5);
        usage.cost_usd = Some(0.0042);
        usage.ttft_ms = Some(125.0);
        usage.output_tokens_per_second = Some(40.0);
        let update = update_for_event(
            StreamEvent::MessageDelta {
                stop_reason: None,
                usage: Some(usage),
            },
            &streamed,
        );
        let value = serde_json::to_value(update).unwrap();
        assert_eq!(value["sessionUpdate"], "usage_update");
        assert_eq!(value["cost"]["amount"], 0.0042);
        assert_eq!(
            value["_meta"]["rung"]["MessageDelta"]["usage"]["ttft_ms"],
            125.0
        );
        assert_eq!(
            value["_meta"]["rung"]["MessageDelta"]["usage"]["cache_read_input_tokens"],
            80
        );
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

    fn failed(kind: Option<FailureKind>) -> JobError {
        JobError {
            reason: "why".into(),
            kind,
        }
    }

    fn wire<T: serde::Serialize>(v: T) -> Value {
        serde_json::to_value(v).unwrap()
    }

    /// An interrupt, or any failure after `session/cancel`, is `cancelled`
    /// with nothing else: ACP says a cancel MUST end as `cancelled`.
    #[test]
    fn a_cancelled_failure_is_plain_cancelled() {
        let plain = serde_json::json!({"stopReason": "cancelled"});
        let r = prompt_failure(failed(Some(FailureKind::Interrupted)), false).unwrap();
        assert_eq!(wire(r), plain);
        let r = prompt_failure(failed(Some(FailureKind::Provider)), true).unwrap();
        assert_eq!(wire(r), plain);
        let r = prompt_failure(failed(None), true).unwrap();
        assert_eq!(wire(r), plain);
    }

    /// A failure before the loop ran is not a turn's end: prose data, as before.
    #[test]
    fn a_setup_failure_keeps_its_prose_data() {
        let e = prompt_failure(failed(None), false).unwrap_err();
        assert_eq!(
            wire(e),
            serde_json::json!({"code": -32603, "message": "Internal error", "data": "why"})
        );
    }

    /// Every loop failure kind has a wire state; only cap and refusal are results.
    #[test]
    fn every_failure_kind_has_a_terminal_state() {
        use FailureKind::*;
        let cases = [
            (MaxIterations, "cap_exhausted", None),
            (BudgetExhausted, "cap_exhausted", None),
            (Refusal, "refused", None),
            (Overflow, "overflow", None),
            (DoomLoop, "doom_loop", None),
            (ContentPolicy, "failed", Some("content_policy")),
            (Auth, "failed", Some("auth")),
            (Forbidden, "failed", Some("forbidden")),
            (Quota, "failed", Some("quota")),
            (Config, "failed", Some("config")),
            (Provider, "failed", Some("provider")),
        ];
        for (kind, state, sub) in cases {
            let mut terminal = serde_json::json!({"state": state, "reason": "why"});
            if let Some(k) = sub {
                terminal["kind"] = k.into();
            }
            let meta = serde_json::json!({"rung": {"terminal": terminal}});
            let got = match prompt_failure(failed(Some(kind)), false) {
                Ok(r) => wire(r),
                Err(e) => wire(e),
            };
            let want = match kind {
                MaxIterations | BudgetExhausted => {
                    serde_json::json!({"stopReason": "max_turn_requests", "_meta": meta})
                }
                Refusal => serde_json::json!({"stopReason": "refusal", "_meta": meta}),
                _ => serde_json::json!({"code": -32603, "message": "Internal error", "data": meta}),
            };
            assert_eq!(got, want, "{kind:?}");
        }
    }
}
