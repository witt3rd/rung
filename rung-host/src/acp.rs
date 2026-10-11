//! ACP outward: one agent, many channels.
//!
//! `session/new` opens a **channel** to the one agent — an owner, peer or
//! observer channel — never a new agent. The transport's principal caps the
//! role a channel may take (stdio: the operator's configured role).
//!
//! - `session/prompt` is a stimulus from that channel's principal, recorded
//!   and fsynced before anything else happens. It is answered when the turn
//!   that disposes it ends: the agent's sends to that channel in that turn
//!   are streamed to it as `agent_message_chunk`s (or the turn's final text
//!   when it sent nothing), the turn's tool calls as `tool_call`s, and the
//!   response's `_meta.rung` names `{item, turn, disposition,
//!   admitted_with}`. Several channels' prompts may share one turn; each
//!   still gets exactly one response. A channel sees only output of work it
//!   owns: when a turn admitted prompts from several channels, the final
//!   text and tool calls go only to the highest-role channel among them
//!   (owner > peer > observer; every session of that channel), and any
//!   other channel gets only what the agent sends to it. `admitted_with`
//!   lists only item ids of the same channel.
//! - An agent-initiated message (a send with no open prompt from that
//!   channel in its turn) goes to the channel's clients as an `_rung/outbox`
//!   notification when they opted in at `initialize`
//!   (`_meta.rung.outbox: true`), otherwise at the head of the channel's
//!   next response.
//! - `session/cancel` withdraws the channel's pending stimulus (disposition
//!   `withdrawn`); one already in the running turn is answered `cancelled`,
//!   and an owner's cancel also cuts that turn.
//! - `session/list` lists the channels the record holds; `session/load`
//!   reopens one.
//! - Extensions: `_rung/status` (the now set; any role), `_rung/stimulus` (a
//!   stimulus that asks no reply; owner or peer; durable before its ack),
//!   and owner only: `_rung/report` (plain-text what-are-you-doing report,
//!   no model call), `_rung/stop`, `_rung/release`, `_rung/calendar`.
//!
//! The bridge reads the record through a [`crate::core::Core::observe`]
//! hook; it never decides anything the loop decides.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::v1::{
    AgentCapabilities, CancelNotification, ContentBlock, ContentChunk, Implementation,
    InitializeRequest, InitializeResponse, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest,
    PromptResponse, SessionCapabilities, SessionId, SessionInfo, SessionListCapabilities,
    SessionNotification, SessionUpdate, StopReason, TextContent, ToolCall, ToolCallStatus,
};
use agent_client_protocol::{
    Agent, Client, ConnectTo, ConnectionTo, Error, Responder, Result as AcpResult, Stdio,
    UntypedMessage,
};
use serde_json::{Map, Value, json};

use crate::calendar::{Entry, Missed, Origin, When};
use crate::clock::SECOND;
use crate::core::lock;
use crate::inbox::{Item, Role};
use crate::presence::Host;
use crate::record::{Line, Record};
use crate::redact::Redactor;

/// Who is on the other end of a transport: the highest role its channels
/// may take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Principal {
    pub role: Role,
}

fn rank(r: Role) -> u8 {
    match r {
        Role::Owner => 3,
        Role::Peer => 2,
        Role::Observer => 1,
        Role::Host => 0,
    }
}

fn role_name(r: Role) -> &'static str {
    match r {
        Role::Owner => "owner",
        Role::Peer => "peer",
        Role::Observer => "observer",
        Role::Host => "host",
    }
}

fn parse_role(s: &str) -> Option<Role> {
    match s {
        "owner" => Some(Role::Owner),
        "peer" => Some(Role::Peer),
        "observer" => Some(Role::Observer),
        _ => None,
    }
}

fn refuse(msg: impl Into<String>) -> Error {
    Error::invalid_request().data(msg.into())
}

fn bad(msg: impl Into<String>) -> Error {
    Error::invalid_params().data(msg.into())
}

/// One open channel on one connection.
struct Channel {
    role: Role,
    channel: String,
    conn: ConnectionTo<Client>,
    /// Its client takes `_rung/outbox` notifications.
    outbox: bool,
}

/// A prompt waiting for the turn that disposes its item.
struct Prompt {
    session: String,
    channel: String,
    responder: Responder<PromptResponse>,
    /// The turn it is in, once admitted.
    turn: Option<u64>,
    /// Text was streamed to it.
    streamed: bool,
}

#[derive(Default)]
struct Bridge {
    sessions: HashMap<String, Channel>,
    /// By item id.
    prompts: BTreeMap<String, Prompt>,
    /// Agent-initiated messages held for a session's next response.
    held: HashMap<String, Vec<String>>,
    /// turn → items disposed in it: (id, disposition).
    disposed: BTreeMap<u64, Vec<(String, String)>>,
    /// Every accepted item's role and channel, by item id.
    items: HashMap<String, (Role, String)>,
    /// turn → the channel owning its work: the highest role among all
    /// admitted non-host items, prompted or not.
    owner: BTreeMap<u64, String>,
    /// turn → every prompt admitted in it: (item, channel).
    batch: BTreeMap<u64, Vec<(String, String)>>,
}

impl Bridge {
    /// The channel that owns a turn's work: the highest role among the
    /// channels with a prompt admitted in it. Only it sees the turn's final
    /// text and tool calls; every other channel sees only what the agent
    /// sends to it.
    fn owning_channel(&self, turn: u64) -> Option<String> {
        self.owner.get(&turn).cloned()
    }

    fn text(&self, session: &str, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(ch) = self.sessions.get(session) {
            let _ = ch.conn.send_notification(SessionNotification::new(
                SessionId::new(session.to_string()),
                SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                    TextContent::new(text),
                ))),
            ));
        }
    }

    fn flush_held(&mut self, session: &str) {
        if let Some(held) = self.held.remove(session) {
            for t in held {
                self.text(session, &t);
            }
        }
    }

    fn respond(&mut self, id: &str, reason: StopReason, meta: Value) {
        let Some(p) = self.prompts.remove(id) else {
            return;
        };
        let Value::Object(mut m) = meta else { return };
        m.insert("item".into(), json!(id));
        let mut outer = Map::new();
        outer.insert("rung".into(), Value::Object(m));
        let _ = p.responder.respond(PromptResponse::new(reason).meta(outer));
    }

    fn on_line(&mut self, l: &Line) {
        match l.kind.as_str() {
            "stimulus.accepted" => {
                let it = l.get("item");
                if let (Some(id), Some(ch), Ok(role)) = (
                    it["id"].as_str(),
                    it["channel"].as_str(),
                    serde_json::from_value::<Role>(it["role"].clone()),
                ) {
                    self.items.insert(id.to_string(), (role, ch.to_string()));
                }
            }
            "stimulus.admitted" => {
                let turn = l.u64("turn");
                let ids: Vec<String> = crate::inbox::ids(l.get("ids"))
                    .into_iter()
                    .chain(crate::inbox::ids(l.get("digests")))
                    .collect();
                let top = ids
                    .iter()
                    .filter_map(|id| self.items.get(id))
                    .filter(|(r, _)| *r != Role::Host)
                    .fold(None::<&(Role, String)>, |best, x| match best {
                        Some(b) if rank(b.0) >= rank(x.0) => Some(b),
                        _ => Some(x),
                    })
                    .map(|(_, c)| c.clone());
                match top {
                    Some(c) => self.owner.insert(turn, c),
                    None => self.owner.remove(&turn),
                };
                for id in &ids {
                    if let Some(p) = self.prompts.get_mut(id) {
                        p.turn = Some(turn);
                        let c = p.channel.clone();
                        self.batch.entry(turn).or_default().push((id.clone(), c));
                    }
                }
            }
            "stimulus.requeued" => {
                for id in crate::inbox::ids(l.get("ids")) {
                    if let Some(p) = self.prompts.get_mut(&id) {
                        p.turn = None;
                    }
                }
            }
            "stimulus.disposed" => {
                let id = l.str("id").to_string();
                let d = l.str("disposition").to_string();
                self.items.remove(&id);
                if d == "withdrawn" {
                    self.respond(&id, StopReason::Cancelled, json!({"disposition": d}));
                } else if self.prompts.contains_key(&id) {
                    self.disposed
                        .entry(l.u64("turn"))
                        .or_default()
                        .push((id, d));
                }
            }
            "tool.call" => {
                let turn = l.u64("turn");
                let name = l.str("name").to_string();
                let status = if l.get("ok") == &Value::Bool(true) {
                    ToolCallStatus::Completed
                } else {
                    ToolCallStatus::Failed
                };
                let owner = self.owning_channel(turn);
                let sessions: Vec<String> = self
                    .prompts
                    .values()
                    .filter(|p| p.turn == Some(turn) && Some(&p.channel) == owner.as_ref())
                    .map(|p| p.session.clone())
                    .collect();
                for s in sessions {
                    if let Some(ch) = self.sessions.get(&s) {
                        let _ = ch.conn.send_notification(SessionNotification::new(
                            SessionId::new(s.clone()),
                            SessionUpdate::ToolCall(
                                ToolCall::new(format!("t{turn}-{}", l.seq), name.clone())
                                    .status(status),
                            ),
                        ));
                    }
                }
            }
            "outbox.queued" => {
                let turn = l.u64("turn");
                let channel = l.str("channel").to_string();
                let text = l.str("text").to_string();
                let open: Vec<String> = self
                    .prompts
                    .iter()
                    .filter(|(_, p)| p.turn == Some(turn) && p.channel == channel)
                    .map(|(id, _)| id.clone())
                    .collect();
                if open.is_empty() {
                    // Agent-initiated: to every client of the channel.
                    let targets: Vec<(String, bool)> = self
                        .sessions
                        .iter()
                        .filter(|(_, c)| c.channel == channel)
                        .map(|(s, c)| (s.clone(), c.outbox))
                        .collect();
                    for (s, opted) in targets {
                        if opted {
                            let ch = &self.sessions[&s];
                            if let Ok(n) = UntypedMessage::new(
                                "_rung/outbox",
                                json!({"sessionId": s, "channel": channel, "text": text,
                                       "turn": turn, "source": l.str("source")}),
                            ) {
                                let _ = ch.conn.send_notification(n);
                            }
                        } else {
                            self.held.entry(s).or_default().push(text.clone());
                        }
                    }
                    return;
                }
                let mut done = Vec::new();
                for id in open {
                    let s = self.prompts[&id].session.clone();
                    if !done.contains(&s) {
                        self.flush_held(&s);
                        self.text(&s, &text);
                        done.push(s);
                    }
                    if let Some(p) = self.prompts.get_mut(&id) {
                        p.streamed = true;
                    }
                }
            }
            "turn.ended" => {
                let turn = l.u64("turn");
                let final_text = l.str("final_text").to_string();
                let status = l.str("status").to_string();
                let batch = self.batch.remove(&turn).unwrap_or_default();
                let owner = self.owner.remove(&turn);
                for (id, d) in self.disposed.remove(&turn).unwrap_or_default() {
                    let Some(p) = self.prompts.get(&id) else {
                        continue;
                    };
                    let (s, streamed) = (p.session.clone(), p.streamed);
                    let mine = Some(&p.channel) == owner.as_ref();
                    let channel = p.channel.clone();
                    self.flush_held(&s);
                    if !streamed && mine {
                        self.text(&s, &final_text);
                    }
                    let with: Vec<&String> = batch
                        .iter()
                        .filter(|(x, c)| *x != id && *c == channel)
                        .map(|(x, _)| x)
                        .collect();
                    self.respond(
                        &id,
                        StopReason::EndTurn,
                        json!({"turn": turn, "disposition": d, "status": status,
                               "admitted_with": with}),
                    );
                }
                self.disposed.retain(|t, _| *t > turn);
            }
            "halted" => {
                let ids: Vec<String> = self.prompts.keys().cloned().collect();
                for id in ids {
                    self.respond(
                        &id,
                        StopReason::Cancelled,
                        json!({"disposition": "open", "halted": true, "why": l.get("why")}),
                    );
                }
            }
            _ => {}
        }
    }
}

/// The ACP surface of one host.
pub struct Acp {
    host: Arc<Host>,
    bridge: Arc<Mutex<Bridge>>,
    counter: AtomicU64,
}

impl std::fmt::Debug for Acp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Acp").finish()
    }
}

impl Acp {
    /// Attach to `host`: from now on the bridge sees every record line.
    pub fn attach(host: Arc<Host>) -> Arc<Self> {
        let bridge = Arc::new(Mutex::new(Bridge::default()));
        let (tx, rx) = mpsc::channel::<Line>();
        let tx = Mutex::new(tx);
        host.core.observe(Box::new(move |l: &Line| {
            let _ = lock(&tx).send(l.clone());
        }));
        let b2 = bridge.clone();
        std::thread::spawn(move || {
            for l in rx {
                lock(&b2).on_line(&l);
            }
        });
        let seed = host.core.now().unsigned_abs();
        Arc::new(Self {
            host,
            bridge,
            counter: AtomicU64::new(seed),
        })
    }

    fn fresh(&self, prefix: &str) -> String {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("{prefix}-{n:x}-{}", std::process::id())
    }

    fn channel_of(&self, session: &str) -> Option<(Role, String)> {
        let b = lock(&self.bridge);
        b.sessions.get(session).map(|c| (c.role, c.channel.clone()))
    }

    /// The channels the record holds: session → (role, channel).
    fn recorded_channels(&self) -> BTreeMap<String, (Role, String)> {
        let mut out = BTreeMap::new();
        for l in Record::read_dir(self.host.core.record_dir()).unwrap_or_default() {
            if l.kind == "channel.opened"
                && let Some(role) = parse_role(l.str("role"))
            {
                out.insert(
                    l.str("session").to_string(),
                    (role, l.str("channel").to_string()),
                );
            }
        }
        out
    }

    fn item(&self, role: Role, channel: &str, text: &str) -> Item {
        Item::message(
            &self.fresh("acp"),
            role,
            channel,
            self.host.core.now(),
            text,
        )
    }

    fn owner_channel(&self) -> String {
        self.host.core.config.owner_channel.clone()
    }

    /// Serve one connection until it closes.
    pub async fn serve(
        self: Arc<Self>,
        principal: Principal,
        transport: impl ConnectTo<Agent> + 'static,
    ) -> AcpResult<()> {
        let opted = Arc::new(AtomicBool::new(false));
        let me = self.clone();
        let o2 = opted.clone();
        let me2 = self.clone();
        let o3 = opted.clone();
        let me3 = self.clone();
        let o4 = opted.clone();
        let me4 = self.clone();
        let me5 = self.clone();
        let me6 = self.clone();
        Agent
            .builder()
            .name("rung-host")
            .on_receive_request(
                async move |request: InitializeRequest,
                            responder: Responder<InitializeResponse>,
                            _connection: ConnectionTo<Client>| {
                    let wants = request
                        .meta
                        .as_ref()
                        .and_then(|m| m.get("rung"))
                        .and_then(|r| r.get("outbox"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    o2.store(wants, Ordering::SeqCst);
                    responder.respond(
                        InitializeResponse::new(request.protocol_version)
                            .agent_capabilities(
                                AgentCapabilities::new()
                                    .load_session(true)
                                    .session_capabilities(
                                        SessionCapabilities::new()
                                            .list(SessionListCapabilities::new()),
                                    ),
                            )
                            .agent_info(
                                Implementation::new("rung-host", env!("CARGO_PKG_VERSION"))
                                    .title("rung-host"),
                            ),
                    )
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: NewSessionRequest,
                            responder: Responder<NewSessionResponse>,
                            connection: ConnectionTo<Client>| {
                    let rung = request
                        .meta
                        .as_ref()
                        .and_then(|m| m.get("rung"))
                        .cloned()
                        .unwrap_or(Value::Null);
                    let role = match rung["role"].as_str() {
                        None => principal.role,
                        Some(r) => match parse_role(r) {
                            Some(r) => r,
                            None => {
                                return responder
                                    .respond_with_error(bad(format!("unknown role `{r}`")));
                            }
                        },
                    };
                    if rank(role) > rank(principal.role) {
                        return responder.respond_with_error(refuse(format!(
                            "this transport may open {} channels at most",
                            role_name(principal.role)
                        )));
                    }
                    let name = rung["channel"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .unwrap_or("acp");
                    let channel = match role {
                        Role::Owner => me.owner_channel(),
                        other => format!("{}:{name}", role_name(other)),
                    };
                    let session = me.fresh("ch");
                    me.host.open_channel(&session, role, &channel);
                    lock(&me.bridge).sessions.insert(
                        session.clone(),
                        Channel {
                            role,
                            channel,
                            conn: connection,
                            outbox: o3.load(Ordering::SeqCst),
                        },
                    );
                    responder.respond(NewSessionResponse::new(SessionId::new(session)))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: LoadSessionRequest,
                            responder: Responder<LoadSessionResponse>,
                            connection: ConnectionTo<Client>| {
                    let session = request.session_id.0.to_string();
                    let Some((role, channel)) = me2.recorded_channels().get(&session).cloned()
                    else {
                        return responder
                            .respond_with_error(bad(format!("no channel `{session}`")));
                    };
                    if rank(role) > rank(principal.role) {
                        return responder.respond_with_error(refuse(
                            "this transport may not reopen that channel",
                        ));
                    }
                    // The channel's last message from the agent, as history.
                    let last = Record::read_dir(me2.host.core.record_dir())
                        .unwrap_or_default()
                        .into_iter()
                        .rev()
                        .find(|l| l.kind == "outbox.queued" && l.str("channel") == channel)
                        .map(|l| l.str("text").to_string());
                    let mut b = lock(&me2.bridge);
                    b.sessions.insert(
                        session.clone(),
                        Channel {
                            role,
                            channel,
                            conn: connection,
                            outbox: o4.load(Ordering::SeqCst),
                        },
                    );
                    if let Some(t) = last {
                        b.text(&session, &t);
                    }
                    drop(b);
                    responder.respond(LoadSessionResponse::new())
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |_request: ListSessionsRequest,
                            responder: Responder<ListSessionsResponse>,
                            _connection: ConnectionTo<Client>| {
                    let cwd = me3.host.core.config.workspace.clone();
                    let sessions = me3
                        .recorded_channels()
                        .into_iter()
                        .filter(|(_, (r, _))| rank(*r) <= rank(principal.role))
                        .map(|(s, (r, c))| {
                            SessionInfo::new(SessionId::new(s), cwd.clone())
                                .title(format!("{} · {c}", role_name(r)))
                        })
                        .collect();
                    responder.respond(ListSessionsResponse::new(sessions))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: PromptRequest,
                            responder: Responder<PromptResponse>,
                            _connection: ConnectionTo<Client>| {
                    let session = request.session_id.0.to_string();
                    let Some((role, channel)) = me4.channel_of(&session) else {
                        return responder
                            .respond_with_error(bad(format!("no channel `{session}`")));
                    };
                    if role == Role::Observer {
                        return responder
                            .respond_with_error(refuse("an observer channel cannot prompt"));
                    }
                    let text: Vec<String> = request
                        .prompt
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text(t) => Some(t.text.clone()),
                            _ => None,
                        })
                        .collect();
                    let item = me4.item(role, &channel, &text.join("\n"));
                    // Registered before it is recorded, so no line about it
                    // can pass the bridge unseen.
                    lock(&me4.bridge).prompts.insert(
                        item.id.clone(),
                        Prompt {
                            session,
                            channel,
                            responder,
                            turn: None,
                            streamed: false,
                        },
                    );
                    me4.host.accept_external(&item);
                    Ok(())
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                async move |n: CancelNotification, _connection: ConnectionTo<Client>| {
                    let session = n.session_id.0.to_string();
                    let role = me5.channel_of(&session).map(|c| c.0);
                    let ids: Vec<(String, bool)> = {
                        let b = lock(&me5.bridge);
                        b.prompts
                            .iter()
                            .filter(|(_, p)| p.session == session)
                            .map(|(id, p)| (id.clone(), p.turn.is_some()))
                            .collect()
                    };
                    for (id, in_turn) in ids {
                        // Withdrawn: the bridge answers it from the record line.
                        if !in_turn && me5.host.withdraw(&id) {
                            continue;
                        }
                        if role == Some(Role::Owner) {
                            me5.host.cut_turn();
                        }
                        lock(&me5.bridge).respond(
                            &id,
                            StopReason::Cancelled,
                            json!({"disposition": "in_turn"}),
                        );
                    }
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |request: UntypedMessage,
                            responder: Responder<Value>,
                            _connection: ConnectionTo<Client>| {
                    match me6.extension(&request.method, &request.params) {
                        Ok(v) => responder.respond(v),
                        Err(e) => responder.respond_with_error(e),
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_to(transport)
            .await
    }

    /// The `_rung/*` extension requests.
    fn extension(&self, method: &str, params: &Value) -> Result<Value, Error> {
        if !method.starts_with("_rung/") {
            return Err(Error::method_not_found());
        }
        let session = params["sessionId"]
            .as_str()
            .ok_or_else(|| bad("sessionId is required"))?;
        let (role, channel) = self
            .channel_of(session)
            .ok_or_else(|| bad(format!("no channel `{session}`")))?;
        let owner_only = |what: &str| -> Result<(), Error> {
            if role == Role::Owner {
                Ok(())
            } else {
                Err(refuse(format!("{what} is the owner's")))
            }
        };
        match method {
            "_rung/status" => Ok(self.host.status()),
            "_rung/report" => {
                owner_only("_rung/report")?;
                Ok(json!({"text": self.host.report()}))
            }
            "_rung/stimulus" => {
                if role == Role::Observer {
                    return Err(refuse("an observer channel cannot send stimuli"));
                }
                let text = params["text"]
                    .as_str()
                    .ok_or_else(|| bad("text is required"))?;
                let mut item = self.item(role, &channel, text);
                item.urgency = params["urgency"].as_str().map(str::to_string);
                self.host.accept_external(&item);
                Ok(json!({"id": item.id}))
            }
            "_rung/stop" => {
                owner_only("_rung/stop")?;
                let mut item = self.item(role, &channel, "stop");
                item.control = Some("stop".into());
                self.host.accept_external(&item);
                Ok(json!({"stopping": true}))
            }
            "_rung/release" => {
                owner_only("_rung/release")?;
                let committed = self.host.core.state().kernel.commitment().is_some();
                let reason = params["reason"].as_str().unwrap_or("the owner released it");
                let mut item = self.item(role, &channel, reason);
                item.control = Some("release".into());
                self.host.accept_external(&item);
                Ok(json!({"released": committed}))
            }
            "_rung/calendar" => {
                owner_only("_rung/calendar")?;
                let id = params["id"].as_str().ok_or_else(|| bad("id is required"))?;
                let text = params["text"]
                    .as_str()
                    .ok_or_else(|| bad("text is required"))?;
                let at = match (params["at"].as_i64(), params["in_s"].as_i64()) {
                    (Some(at), _) => at,
                    (None, Some(s)) => self.host.core.now() + s * SECOND,
                    _ => return Err(bad("at (ms since the epoch) or in_s is required")),
                };
                let entry = Entry {
                    id: id.to_string(),
                    when: When::At(at),
                    origin: Origin::Owner,
                    text: text.to_string(),
                    firm: params["firm"].as_bool().unwrap_or(false),
                    missed: Missed::OnceLate,
                };
                self.host.add_calendar(&entry);
                Ok(json!({"id": id, "at": at}))
            }
            _ => Err(Error::method_not_found()),
        }
    }
}

/// Serve one local client on stdio until it closes.
pub fn serve_stdio(acp: Arc<Acp>, principal: Principal) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(acp.serve(principal, Stdio::new()))
        .map_err(|e| e.to_string())
}

/// Serve ACP over Streamable HTTP on `addr` (loopback by default): each
/// bearer token is the operator's, named per role in `tokens`
/// (token → role). A connection's principal is the role of the token it
/// initialized with; every later request on it must carry the same token.
/// A request with no known token is refused (401). At least one token is
/// required: an unauthenticated HTTP surface is not served.
///
/// The same listener carries the host's own doors beside `/acp`; today one:
/// `GET /v1/events`, the host's event stream with replay (H5).
pub fn serve_http(acp: Arc<Acp>, addr: &str, tokens: Vec<(String, Role)>) -> Result<(), String> {
    serve_http_bound(acp, addr, tokens, |_| {})
}

/// [`serve_http`], telling `on_bound` the address actually bound (the real
/// port when `addr` asked for port 0) before the first connection is served.
pub fn serve_http_bound(
    acp: Arc<Acp>,
    addr: &str,
    tokens: Vec<(String, Role)>,
    on_bound: impl FnOnce(std::net::SocketAddr),
) -> Result<(), String> {
    serve_http_with(acp, addr, tokens, Redactor::new(), Vec::new(), on_bound)
}

/// [`serve_http_bound`], with the redactor the host's own doors pass every
/// frame through: it removes the value of every key the host's config names
/// by variable, as well as the known key shapes (and, always, the host's own
/// tokens). `env_unset` names the variables the config names that this
/// process does not hold (their values cannot be found by value); the stream's
/// first frame carries them.
pub fn serve_http_with(
    acp: Arc<Acp>,
    addr: &str,
    tokens: Vec<(String, Role)>,
    mut red: Redactor,
    env_unset: Vec<String>,
    on_bound: impl FnOnce(std::net::SocketAddr),
) -> Result<(), String> {
    if tokens.is_empty() {
        return Err("ACP over HTTP needs at least one role token".into());
    }
    for (t, _) in &tokens {
        red.add_secret(t);
    }
    let door = Arc::new(EventsDoor {
        state_dir: acp
            .host
            .core
            .record_dir()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
        bus: acp.host.live(),
        red,
        env_unset,
        tokens: tokens.iter().map(|(t, _)| t.clone()).collect(),
    });
    let tokens: Arc<HashMap<String, Role>> = Arc::new(tokens.into_iter().collect());
    let t2 = tokens.clone();
    let serve = rung_agent_core::acp_http::Serve {
        name: "rung-host".into(),
        authorize: Arc::new(move |bearer| bearer.is_some_and(|b| t2.contains_key(b))),
        connect: Arc::new(move |bearer, lines| {
            let acp = acp.clone();
            let role = bearer.and_then(|b| tokens.get(&b).copied());
            Box::pin(async move {
                if let Some(role) = role {
                    let _ = acp.serve(Principal { role }, lines).await;
                }
            })
        }),
        bind_bearer: true,
        routes: Some(door.routes()),
    };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(rung_agent_core::acp_http::listen_bound(
        addr.to_string(),
        serve,
        on_bound,
    ))
}

/// The host's own doors on its listener (H5).
pub struct EventsDoor {
    /// The state directory whose record the door follows.
    pub state_dir: PathBuf,
    pub bus: Arc<crate::stream::LiveBus>,
    pub red: Redactor,
    /// Variables the config names that this process does not hold, sorted.
    pub env_unset: Vec<String>,
    /// The tokens that may read: any role's.
    pub tokens: std::collections::HashSet<String>,
}

impl EventsDoor {
    /// `GET /v1/events`: the record from the client's cursor, then as it is
    /// written, with the running turn's partial frames. Any of the host's role
    /// tokens may read it (a token that names no role is refused 401); every
    /// frame is redacted. Shapes: `docs/rung-host-api.md` (H5).
    pub fn routes(self: &Arc<Self>) -> rung_agent_core::acp_http::Routes {
        use rung_agent_core::acp_http::{RouteBody, RouteReply, RouteRequest};
        let door = self.clone();
        Arc::new(move |req: RouteRequest| {
            let door = door.clone();
            Box::pin(async move {
                let context = crate::context::turn_of_path(&req.path).map(str::to_string);
                if req.path != "/v1/events" && context.is_none() {
                    return None;
                }
                let error = |status: u16, code: &str, message: &str| {
                    let body = json!({"error": code, "message": message});
                    Some(RouteReply {
                        status,
                        headers: vec![("content-type".into(), "application/json".into())],
                        body: RouteBody::Bytes(body.to_string().into_bytes()),
                    })
                };
                let header = |n: &str| {
                    req.headers
                        .iter()
                        .find(|(k, _)| k == n)
                        .map(|(_, v)| v.as_str())
                };
                let bearer = header("authorization").and_then(|v| v.strip_prefix("Bearer "));
                if !bearer.is_some_and(|b| door.tokens.contains(b)) {
                    return error(401, "unauthorized", "the token is not recognised");
                }
                let head = req.method == "HEAD";
                if req.method != "GET" && !head {
                    return error(405, "method_not_allowed", "this door only reads");
                }
                let Some(pairs) = crate::stream::query_pairs(req.query.as_deref()) else {
                    return error(400, "bad_request", "the query is not UTF-8");
                };
                if let Some(turn) = context {
                    return match door.context_page(&turn, &pairs, head).await {
                        Ok(r) => Some(r),
                        Err(e) => error(e.status, e.code, &door.red.redact(&e.message)),
                    };
                }
                let q = match crate::stream::EventsQuery::parse(&pairs, header("last-event-id")) {
                    Ok(q) => q,
                    Err(e) => return error(e.status, e.code, &e.message),
                };
                let mut o = crate::stream::Options::new(door.red.clone());
                o.after = q.after;
                o.partials = q.partials;
                o.env_unset = door.env_unset.clone();
                o.bus = Some(door.bus.clone());
                match crate::stream::Events::open(&door.state_dir, o).await {
                    // HEAD: the headers of a stream, and no body
                    Ok(_) if head => Some(RouteReply {
                        status: 200,
                        headers: crate::stream::sse_headers(),
                        body: RouteBody::Bytes(Vec::new()),
                    }),
                    Ok(ev) => Some(RouteReply {
                        status: 200,
                        headers: crate::stream::sse_headers(),
                        body: RouteBody::Stream(Box::new(ev)),
                    }),
                    Err(e) => error(e.status, e.code, &e.message),
                }
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send>>
        })
    }
}

/// How long a client of a streamed page may take no chunk before it is
/// dropped (the one blocking thread that writes its page is then freed).
pub const PAGE_STALL: std::time::Duration = std::time::Duration::from_secs(30);

/// Bytes written to a page go out in chunks of about this size.
const PAGE_CHUNK: usize = 64 * 1024;

/// A writer that hands what it is given to a channel in chunks; the channel
/// holds two, so a slow client holds back the read instead of the read
/// filling memory.
struct PageWriter {
    tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    buf: Vec<u8>,
    rt: tokio::runtime::Handle,
}

impl PageWriter {
    fn send(&mut self) -> std::io::Result<()> {
        use tokio::sync::mpsc::error::SendTimeoutError;
        if self.buf.is_empty() {
            return Ok(());
        }
        let chunk = std::mem::replace(&mut self.buf, Vec::with_capacity(PAGE_CHUNK));
        match self.rt.block_on(self.tx.send_timeout(chunk, PAGE_STALL)) {
            Ok(()) => Ok(()),
            Err(SendTimeoutError::Timeout(_)) => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "client stalled",
            )),
            Err(SendTimeoutError::Closed(_)) => Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "client gone",
            )),
        }
    }
}

impl std::io::Write for PageWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(data);
        if self.buf.len() >= PAGE_CHUNK {
            self.send()?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.send()
    }
}

/// The chunks of a page, as a response body: ends when the writer is done or
/// gave up (a page that was cut is not a complete JSON document).
struct PageStream {
    rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
}

impl rung_agent_core::acp_http::BodyStream for PageStream {
    fn next(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Vec<u8>>> + Send + '_>> {
        Box::pin(self.rx.recv())
    }
}

impl EventsDoor {
    /// `GET /v1/turns/{n}/context`: what was sent to the model for one call of
    /// one turn, read from the record's files and streamed, a band at a time
    /// (S4). Refusals are answered before the first byte.
    async fn context_page(
        &self,
        turn: &str,
        pairs: &[(String, String)],
        head: bool,
    ) -> crate::read::Door<rung_agent_core::acp_http::RouteReply> {
        use rung_agent_core::acp_http::{RouteBody, RouteReply};
        let turn = crate::context::parse_turn(turn)?;
        let q = crate::context::ContextQuery::parse(pairs)?;
        let (dir, red, unset) = (
            self.state_dir.clone(),
            self.red.clone(),
            self.env_unset.clone(),
        );
        let page = tokio::task::spawn_blocking(move || {
            crate::context::open_dir(&dir, turn, &q, &red, unset)
        })
        .await
        .map_err(|_| crate::read::DoorError::new(500, "internal", "the read failed"))??;
        let headers = vec![("content-type".to_string(), "application/json".to_string())];
        if head {
            return Ok(RouteReply {
                status: 200,
                headers,
                body: RouteBody::Bytes(Vec::new()),
            });
        }
        let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(2);
        let rt = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let mut w = PageWriter {
                tx,
                buf: Vec::with_capacity(PAGE_CHUNK),
                rt,
            };
            if page.write_json(&mut w).is_ok() {
                let _ = std::io::Write::flush(&mut w);
            }
        });
        Ok(RouteReply {
            status: 200,
            headers,
            body: RouteBody::Stream(Box::new(PageStream { rx })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(seq: u64, kind: &str, body: Value) -> Line {
        let Value::Object(body) = body else { panic!() };
        Line {
            seq,
            at: 0,
            kind: kind.into(),
            body,
        }
    }

    #[test]
    fn bridge_forgets_items_and_owners_once_a_turn_is_over() {
        let mut b = Bridge::default();
        let mut seq = 0;
        let mut next = || {
            seq += 1;
            seq
        };
        for turn in 1..=200u64 {
            let ids: Vec<String> = (0..3).map(|i| format!("i{turn}-{i}")).collect();
            for id in &ids {
                let it = json!({"id": id, "role": "owner", "channel": "owner"});
                b.on_line(&line(next(), "stimulus.accepted", json!({"item": it})));
            }
            b.on_line(&line(
                next(),
                "stimulus.admitted",
                json!({"turn": turn, "ids": ids}),
            ));
            assert_eq!(b.owner.get(&turn).map(String::as_str), Some("owner"));
            for id in &ids {
                b.on_line(&line(
                    next(),
                    "stimulus.disposed",
                    json!({"id": id, "turn": turn, "disposition": "answered"}),
                ));
            }
            b.on_line(&line(
                next(),
                "turn.ended",
                json!({"turn": turn, "status": "ok", "final_text": ""}),
            ));
        }
        assert!(b.items.is_empty());
        assert!(b.owner.is_empty());
        assert!(b.batch.is_empty());
        assert!(b.disposed.is_empty());
    }
}
