//! ACP over Streamable HTTP: `POST`/`GET`/`DELETE /acp`, `Acp-Connection-Id`
//! and `Acp-Session-Id`, SSE outbound streams (the experimental RFD that
//! `@agentclientprotocol/sdk` speaks).
//!
//! [`listen_with`] serves any agent: a [`Serve`] says which bearer tokens
//! may reach it and runs one agent connection per `initialize`. The
//! `rung-agent --acp-http` server is one [`Serve`]; a host embedding the
//! crate is another.

use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use futures::SinkExt;
use futures::StreamExt;
use futures::stream::Stream;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::header::{HeaderName, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use serde_json::{Map, Value};
use tokio::net::TcpListener;
use tokio::sync::{Notify, mpsc};

use crate::acp::{Live, connect_agent};
use crate::args::Args;

const HEADER_CONNECTION_ID: &str = "acp-connection-id";
const HEADER_SESSION_ID: &str = "acp-session-id";
const JSON_MIME: &str = "application/json";
const EVENT_STREAM: &str = "text/event-stream";

const SESSION_SCOPED: &[&str] = &[
    "session/cancel",
    "session/close",
    "session/delete",
    "session/fork",
    "session/load",
    "session/prompt",
    "session/resume",
    "session/set_config_option",
    "session/set_mode",
    "session/set_model",
    "session/suggestion/next_edit",
    "session/suggestion/accept",
    "session/suggestion/reject",
    "session/suggestion/close",
    "document/didOpen",
    "document/didChange",
    "document/didClose",
    "document/didSave",
    "document/didFocus",
];

/// One Streamable HTTP connection's transport to its agent: JSON-RPC lines
/// in and out.
pub type HttpLines = agent_client_protocol::Lines<
    futures::sink::SinkMapErr<
        futures::channel::mpsc::UnboundedSender<String>,
        fn(futures::channel::mpsc::SendError) -> std::io::Error,
    >,
    futures::stream::Map<
        futures::channel::mpsc::UnboundedReceiver<String>,
        fn(String) -> std::io::Result<String>,
    >,
>;

/// Serve one connection: given the bearer token it initialized with (if
/// any) and its transport, run the agent until the transport closes.
pub type Connect = Arc<
    dyn Fn(Option<String>, HttpLines) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

/// Whether a request's bearer token (if any) may reach the agent.
pub type Authorize = Arc<dyn Fn(Option<&str>) -> bool + Send + Sync>;

/// A request to a path other than `/acp`, before its body: what a
/// [`RouteGate`] sees.
#[derive(Debug, Clone)]
pub struct RouteHead {
    pub method: String,
    pub path: String,
    /// The raw query string, without the `?`.
    pub query: Option<String>,
    /// Header names lower-cased.
    pub headers: Vec<(String, String)>,
}

/// What a [`RouteGate`] says of a request, from its headers alone.
pub enum GateVerdict {
    /// Answered at once: the body is not read (a client sending one is cut
    /// off), and the connection is closed.
    Refuse(RouteReply),
    /// Let on to its [`Routes`] handler, which gets the body whole.
    Admit,
    /// Not the gate's: the handler is run with no body read.
    Leave,
}

/// Decides from the headers alone, before any body is read, what becomes of a
/// request to a path other than `/acp`. Only an [`GateVerdict::Admit`]ted
/// request has its body read.
pub type RouteGate = Arc<dyn Fn(&RouteHead) -> GateVerdict + Send + Sync>;

/// A request to a path other than `/acp`, as a [`Routes`] handler sees it.
#[derive(Debug, Clone)]
pub struct RouteRequest {
    pub method: String,
    pub path: String,
    /// The raw query string, without the `?`.
    pub query: Option<String>,
    /// Header names lower-cased.
    pub headers: Vec<(String, String)>,
    /// The request body, whole; empty for a GET or HEAD (the listener reads no
    /// body for those, nor for a request the gate did not admit). It is read only after the [`RouteGate`] let the
    /// request on, as it arrives and with no limit on its size; a large one is
    /// spooled to an unlinked temporary file in [`Serve::spool_dir`] as it arrives
    /// (never held whole in memory by the listener), and read back through [`Body`].
    pub body: Body,
}

/// Bodies up to this size stay in memory; larger ones are spooled to disk.
const SPOOL_THRESHOLD: usize = 1 << 20;

/// A request body, whole and immutable: in memory if small, else in an
/// unlinked temporary file in [`Serve::spool_dir`] (`std::env::temp_dir()` if
/// unset; a RAM-backed directory there defeats flat memory). Cheap to clone; each
/// [`Body::reader`] reads independently from the start.
#[derive(Debug, Clone)]
pub struct Body(Repr);

#[derive(Debug, Clone)]
enum Repr {
    Mem(Arc<Vec<u8>>),
    File(Arc<std::fs::File>, u64),
}

impl Default for Body {
    fn default() -> Self {
        Body::from(Vec::new())
    }
}

impl From<Vec<u8>> for Body {
    fn from(v: Vec<u8>) -> Self {
        Body(Repr::Mem(Arc::new(v)))
    }
}

impl Body {
    /// The body's length in bytes.
    pub fn len(&self) -> u64 {
        match &self.0 {
            Repr::Mem(v) => v.len() as u64,
            Repr::File(_, n) => *n,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The body from its first byte, as a reader (a file-backed body is read
    /// in place, not loaded).
    pub fn reader(&self) -> impl std::io::Read + Send + 'static {
        BodyReader {
            body: self.clone(),
            pos: 0,
        }
    }

    /// Is every byte ASCII whitespace (or is there none)? Reads in constant
    /// memory, off the async runtime when the body is on disk.
    pub async fn is_blank(&self) -> std::io::Result<bool> {
        let b = self.clone();
        b.off_runtime(Body::is_blank_sync).await
    }

    async fn off_runtime<T: Send + 'static>(
        self,
        f: impl FnOnce(&Body) -> T + Send + 'static,
    ) -> T {
        match self.0 {
            Repr::Mem(_) => f(&self),
            Repr::File(..) => tokio::task::spawn_blocking(move || f(&self))
                .await
                .expect("body read task"),
        }
    }

    fn is_blank_sync(&self) -> std::io::Result<bool> {
        use std::io::Read;
        let mut r = self.reader();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = r.read(&mut buf)?;
            if n == 0 {
                return Ok(true);
            }
            if !buf[..n].iter().all(u8::is_ascii_whitespace) {
                return Ok(false);
            }
        }
    }

    /// The body parsed as JSON, read through a buffer (no second whole copy),
    /// off the async runtime when the body is on disk. A failed read of the
    /// spool is [`serde_json::Error::is_io`]; anything else is bad JSON.
    pub async fn json(&self) -> Result<Value, serde_json::Error> {
        self.clone()
            .off_runtime(|b| {
                serde_json::from_reader(std::io::BufReader::with_capacity(64 * 1024, b.reader()))
            })
            .await
    }

    /// The whole body in memory (this is the copy the spool avoids; use
    /// [`Body::reader`] where the body can be large).
    pub fn to_vec(&self) -> std::io::Result<Vec<u8>> {
        use std::io::Read;
        let mut out = Vec::with_capacity(self.len() as usize);
        self.reader().read_to_end(&mut out)?;
        Ok(out)
    }
}

struct BodyReader {
    body: Body,
    pos: u64,
}

impl std::io::Read for BodyReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = match &self.body.0 {
            Repr::Mem(v) => {
                let p = (self.pos as usize).min(v.len());
                let n = out.len().min(v.len() - p);
                out[..n].copy_from_slice(&v[p..p + n]);
                n
            }
            Repr::File(f, len) => {
                use std::os::unix::fs::FileExt;
                let want = out.len().min((*len - self.pos.min(*len)) as usize);
                f.read_at(&mut out[..want], self.pos)?
            }
        };
        self.pos += n as u64;
        Ok(n)
    }
}

/// Collects a request body as it arrives: in memory until it passes
/// [`SPOOL_THRESHOLD`], then in an unlinked temporary file.
struct Spool {
    dir: PathBuf,
    mem: Vec<u8>,
    file: Option<std::fs::File>,
    len: u64,
}

impl Spool {
    fn new(hint: usize, dir: PathBuf) -> Self {
        Spool {
            dir,
            mem: Vec::with_capacity(hint.min(SPOOL_THRESHOLD)),
            file: None,
            len: 0,
        }
    }

    fn push(&mut self, data: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        self.len += data.len() as u64;
        if self.file.is_none() && self.mem.len() + data.len() <= SPOOL_THRESHOLD {
            self.mem.extend_from_slice(data);
            return Ok(());
        }
        if self.file.is_none() {
            use std::os::unix::fs::OpenOptionsExt;
            let path = self.dir.join(format!("rung-body-{}", uuid::Uuid::new_v4()));
            let mut f = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)?;
            // unlinked at once: it lives only as long as the handle
            std::fs::remove_file(&path)?;
            f.write_all(&self.mem)?;
            self.mem = Vec::new();
            self.file = Some(f);
        }
        self.file.as_mut().expect("spool file").write_all(data)
    }

    fn finish(self) -> Body {
        match self.file {
            Some(f) => Body(Repr::File(Arc::new(f), self.len)),
            None => Body::from(self.mem),
        }
    }
}

/// A body written in pieces, as the client can take them: the listener asks
/// for the next piece only when it can write one, so a client that does not
/// read stops the producer, and a client that goes away drops it.
pub trait BodyStream: Send {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>> + Send + '_>>;
}

/// What a [`Routes`] handler answers.
pub struct RouteReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: RouteBody,
}

pub enum RouteBody {
    Bytes(Vec<u8>),
    Stream(Box<dyn BodyStream>),
}

/// Answers a request to a path other than `/acp`, or `None` for not found.
/// The handler does its own authorization: it sees the headers.
pub type Routes = Arc<
    dyn Fn(RouteRequest) -> Pin<Box<dyn Future<Output = Option<RouteReply>> + Send>> + Send + Sync,
>;

/// What a Streamable HTTP listener serves.
#[derive(Clone)]
pub struct Serve {
    /// Names the server in its listen line (`<name>: ACP HTTP at …/acp`).
    pub name: String,
    pub authorize: Authorize,
    pub connect: Connect,
    /// Every later request of a connection must carry the bearer token it
    /// initialized with (a connection's principal is fixed at initialize).
    pub bind_bearer: bool,
    /// Paths other than `/acp`, if any (a host's own doors).
    pub routes: Option<Routes>,
    /// Refuses a request to those paths from its headers, before its body is
    /// read (authentication, method, media type). `None`: no early refusal.
    pub route_gate: Option<RouteGate>,
    /// Where a large request body is spooled as it arrives (an unlinked
    /// temporary file). Put it on a disk, next to the host's state: a
    /// RAM-backed directory (tmpfs) holds the body in RAM and defeats flat
    /// memory. `None`: `std::env::temp_dir()`.
    pub spool_dir: Option<PathBuf>,
}

pub(crate) async fn listen(process: Args, addr: String) -> Result<(), String> {
    let token = process.acp_token.clone();
    let process = Arc::new(process);
    let live = Live::new();
    let serve = Serve {
        name: "rung-agent".into(),
        authorize: Arc::new(move |bearer| match token.as_deref() {
            None => true,
            Some(t) => bearer == Some(t),
        }),
        connect: Arc::new(move |_bearer, lines| {
            let process = process.clone();
            let live = live.clone();
            Box::pin(async move {
                let _ = connect_agent(process, live, lines).await;
            })
        }),
        bind_bearer: false,
        routes: None,
        route_gate: None,
        spool_dir: None,
    };
    listen_with(addr, serve).await
}

/// Listen on `addr` and serve each connection with `serve`.
pub async fn listen_with(addr: String, serve: Serve) -> Result<(), String> {
    listen_bound(addr, serve, |_| {}).await
}

/// [`listen_with`], telling `on_bound` the address actually bound (the real
/// port when `addr` asked for port 0) before the first connection is
/// accepted.
pub async fn listen_bound(
    addr: String,
    serve: Serve,
    on_bound: impl FnOnce(SocketAddr),
) -> Result<(), String> {
    let bind = resolve_addr(&addr)?;
    let listener = TcpListener::bind(bind)
        .await
        .map_err(|e| format!("bind {bind}: {e}"))?;
    let local = listener.local_addr().map_err(|e| e.to_string())?;
    on_bound(local);
    rung_std::events::emit(
        "rung-agent",
        "acp.listen",
        &format!("{}: ACP HTTP at http://{local}/acp", serve.name),
    );
    let state = Arc::new(HttpState {
        serve,
        connections: Mutex::new(HashMap::new()),
    });
    loop {
        let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let state = state.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let svc = hyper::service::service_fn(move |req| {
                let state = state.clone();
                async move { handle(state, req).await }
            });
            let _ = AutoBuilder::new(TokioExecutor::new())
                .serve_connection(io, svc)
                .await;
        });
    }
}

struct HttpState {
    serve: Serve,
    connections: Mutex<HashMap<String, Arc<Conn>>>,
}

struct Conn {
    /// The bearer token the connection initialized with.
    bearer: Option<String>,
    to_agent: futures::channel::mpsc::UnboundedSender<String>,
    connection_stream: Mailbox,
    sessions: Mutex<HashMap<String, Mailbox>>,
    pending_routes: Mutex<HashMap<String, Route>>,
    client_response_routes: Mutex<HashMap<String, Route>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Route {
    Connection,
    Session(String),
}

#[derive(Clone)]
struct Mailbox {
    inner: Arc<Mutex<MailInner>>,
    notify: Arc<Notify>,
}

struct MailInner {
    q: VecDeque<Value>,
    finished: bool,
    aborted: bool,
    leased: bool,
}

impl Mailbox {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MailInner {
                q: VecDeque::new(),
                finished: false,
                aborted: false,
                leased: false,
            })),
            notify: Arc::new(Notify::new()),
        }
    }

    fn push(&self, msg: Value) {
        let mut g = self.inner.lock().expect("mailbox");
        if g.finished || g.aborted {
            return;
        }
        g.q.push_back(msg);
        drop(g);
        self.notify.notify_waiters();
    }

    fn try_acquire(&self) -> Option<Lease> {
        let mut g = self.inner.lock().expect("mailbox");
        if g.aborted || g.leased {
            return None;
        }
        g.leased = true;
        Some(Lease {
            mailbox: self.clone(),
        })
    }

    fn finish(&self) {
        let mut g = self.inner.lock().expect("mailbox");
        g.finished = true;
        drop(g);
        self.notify.notify_waiters();
    }

    fn abort(&self) {
        let mut g = self.inner.lock().expect("mailbox");
        g.aborted = true;
        g.finished = true;
        g.q.clear();
        drop(g);
        self.notify.notify_waiters();
    }

    async fn recv(&self, _lease: &Lease) -> Option<Value> {
        loop {
            {
                let mut g = self.inner.lock().expect("mailbox");
                if g.aborted || !g.leased {
                    return None;
                }
                if let Some(v) = g.q.pop_front() {
                    return Some(v);
                }
                if g.finished {
                    return None;
                }
            }
            self.notify.notified().await;
        }
    }

    fn release(&self) {
        let mut g = self.inner.lock().expect("mailbox");
        g.leased = false;
        drop(g);
        self.notify.notify_waiters();
    }
}

struct Lease {
    mailbox: Mailbox,
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.mailbox.release();
    }
}

fn header<'a>(req: &'a Request<Incoming>, name: &str) -> Option<&'a str> {
    req.headers().get(name).and_then(|v| v.to_str().ok())
}

fn is_json_content_type(req: &Request<Incoming>) -> bool {
    header(req, "content-type")
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case(JSON_MIME)
        })
        .unwrap_or(false)
}

fn bearer(req: &Request<Incoming>) -> Option<&str> {
    header(req, "authorization").and_then(|v| v.strip_prefix("Bearer "))
}

fn authorized(state: &HttpState, req: &Request<Incoming>) -> bool {
    (state.serve.authorize)(bearer(req))
}

/// A request on an existing connection carries the connection's principal.
fn same_principal(state: &HttpState, conn: &Conn, req: &Request<Incoming>) -> bool {
    !state.serve.bind_bearer || conn.bearer.as_deref() == bearer(req)
}

async fn handle(
    state: Arc<HttpState>,
    req: Request<Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>, Infallible> {
    Ok(dispatch(state, req).await)
}

async fn dispatch(
    state: Arc<HttpState>,
    req: Request<Incoming>,
) -> Response<BoxBody<Bytes, Infallible>> {
    if req.uri().path() != "/acp" {
        return route(&state, req).await;
    }
    if !authorized(&state, &req) {
        return text(StatusCode::UNAUTHORIZED, "Unauthorized");
    }
    match *req.method() {
        Method::POST => post(state, req).await,
        Method::GET => get(state, req),
        Method::DELETE => delete(state, req),
        _ => text(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed"),
    }
}

/// A path other than `/acp`: the embedding's own route, or not found.
async fn route(state: &HttpState, req: Request<Incoming>) -> Response<BoxBody<Bytes, Infallible>> {
    let Some(routes) = &state.serve.routes else {
        return text(StatusCode::NOT_FOUND, "Not Found");
    };
    let (parts, incoming) = req.into_parts();
    let head = RouteHead {
        method: parts.method.as_str().to_string(),
        path: parts.uri.path().to_string(),
        query: parts.uri.query().map(String::from),
        headers: parts
            .headers
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
            .collect(),
    };
    // refused on its headers: answered at once, the body never read, the
    // connection closed (a client still sending is cut off)
    let admitted = match state.serve.route_gate.as_ref().map(|g| g(&head)) {
        Some(GateVerdict::Refuse(reply)) => {
            let mut r = reply_response(reply);
            r.headers_mut()
                .insert("connection", HeaderValue::from_static("close"));
            return r;
        }
        Some(GateVerdict::Admit) => true,
        Some(GateVerdict::Leave) | None => false,
    };
    // a read has no body to wait for; any other method's body is read here, as
    // it arrives, into a spool (no limit: the
    // gate has already said who may send it)
    let body = if !admitted || matches!(parts.method, Method::GET | Method::HEAD) {
        Body::default()
    } else {
        let hint = parts
            .headers
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let mut spool = Spool::new(
            hint,
            state
                .serve
                .spool_dir
                .clone()
                .unwrap_or_else(std::env::temp_dir),
        );
        let mut incoming = incoming;
        while let Some(frame) = incoming.frame().await {
            match frame {
                Ok(f) => {
                    if let Some(d) = f.data_ref() {
                        let d = d.clone();
                        let pushed =
                            tokio::task::spawn_blocking(move || spool.push(&d).map(|()| spool))
                                .await;
                        match pushed {
                            Ok(Ok(s)) => spool = s,
                            _ => {
                                // the rest of the body is unread: close, so it is
                                // not taken for the next request
                                return closing(text(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Could not spool body",
                                ));
                            }
                        }
                    }
                }
                Err(_) => {
                    return closing(text(StatusCode::BAD_REQUEST, "Invalid request body"));
                }
            }
        }
        spool.finish()
    };
    let request = RouteRequest {
        method: head.method,
        path: head.path,
        query: head.query,
        headers: head.headers,
        body,
    };
    let unread = !admitted && !matches!(parts.method, Method::GET | Method::HEAD);
    let mut response = match routes(request).await {
        Some(reply) => reply_response(reply),
        None => text(StatusCode::NOT_FOUND, "Not Found"),
    };
    if unread {
        response
            .headers_mut()
            .insert("connection", HeaderValue::from_static("close"));
    }
    response
}

/// Marks a response to close the connection (its request body was left unread).
fn closing(mut r: Response<BoxBody<Bytes, Infallible>>) -> Response<BoxBody<Bytes, Infallible>> {
    r.headers_mut()
        .insert("connection", HeaderValue::from_static("close"));
    r
}

/// A route's reply as the listener's response.
fn reply_response(reply: RouteReply) -> Response<BoxBody<Bytes, Infallible>> {
    let mut b = Response::builder().status(reply.status);
    for (k, v) in &reply.headers {
        if let (Ok(k), Ok(v)) = (HeaderName::try_from(k.as_str()), HeaderValue::from_str(v)) {
            b = b.header(k, v);
        }
    }
    let body = match reply.body {
        RouteBody::Bytes(v) => Full::new(Bytes::from(v)).boxed(),
        RouteBody::Stream(s) => {
            // one piece in flight at a time: the producer is asked for the next
            // only when the client has taken the last, and is dropped the moment
            // the client goes away (its pending wait is cut)
            let (tx, rx) = mpsc::channel::<Result<Frame<Bytes>, Infallible>>(1);
            tokio::spawn(async move {
                let mut s = s;
                loop {
                    let piece = tokio::select! {
                        _ = tx.closed() => break,
                        p = s.next() => p,
                    };
                    let Some(piece) = piece else { break };
                    if tx.send(Ok(Frame::data(Bytes::from(piece)))).await.is_err() {
                        break;
                    }
                }
            });
            let stream = BoundedFrames { rx };
            BodyExt::boxed(StreamBody::new(stream))
        }
    };
    b.body(body).expect("response")
}

async fn post(
    state: Arc<HttpState>,
    req: Request<Incoming>,
) -> Response<BoxBody<Bytes, Infallible>> {
    if !is_json_content_type(&req) {
        return text(StatusCode::UNSUPPORTED_MEDIA_TYPE, "Unsupported Media Type");
    }
    let conn_hdr = header(&req, HEADER_CONNECTION_ID).map(str::to_string);
    let sess_hdr = header(&req, HEADER_SESSION_ID).map(str::to_string);
    let bearer_hdr = bearer(&req).map(str::to_string);
    let bound = |conn: &Conn| !state.serve.bind_bearer || conn.bearer == bearer_hdr;
    // the message is parsed from the received chunks in place, not first
    // joined into one more whole copy
    let collected = match req.collect().await {
        Ok(c) => c.aggregate(),
        Err(_) => return text(StatusCode::BAD_REQUEST, "Invalid JSON"),
    };
    let value: Value = match serde_json::from_reader(std::io::BufReader::with_capacity(
        64 * 1024,
        bytes::Buf::reader(collected),
    )) {
        Ok(v) => v,
        Err(_) => return text(StatusCode::BAD_REQUEST, "Invalid JSON"),
    };
    if value.is_array() {
        return text(
            StatusCode::NOT_IMPLEMENTED,
            "Batch JSON-RPC requests are not implemented",
        );
    }
    let Some(obj) = value.as_object() else {
        return text(StatusCode::BAD_REQUEST, "Invalid JSON-RPC message");
    };
    if is_initialize(obj) {
        if conn_hdr.is_some() {
            return text(
                StatusCode::BAD_REQUEST,
                "Initialize not allowed on existing connection",
            );
        }
        return initialize(state.clone(), value, bearer_hdr.clone()).await;
    }
    let Some(conn_id) = conn_hdr else {
        return text(StatusCode::BAD_REQUEST, "Missing Acp-Connection-Id");
    };
    let conn = {
        let g = state.connections.lock().expect("conns");
        g.get(&conn_id).cloned()
    };
    let Some(conn) = conn else {
        return text(StatusCode::NOT_FOUND, "Unknown Acp-Connection-Id");
    };
    if !bound(&conn) {
        return text(StatusCode::FORBIDDEN, "Forbidden");
    }
    match forward(&conn, value, sess_hdr.as_deref()) {
        Ok(()) => empty(StatusCode::ACCEPTED),
        Err((code, msg)) => text(code, msg),
    }
}

fn get(state: Arc<HttpState>, req: Request<Incoming>) -> Response<BoxBody<Bytes, Infallible>> {
    if header(&req, "upgrade")
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
    {
        return text(
            StatusCode::UPGRADE_REQUIRED,
            "WebSocket upgrade is not implemented",
        );
    }
    let accept = header(&req, "accept").unwrap_or("").to_ascii_lowercase();
    if !accept.contains(EVENT_STREAM) {
        return text(StatusCode::NOT_ACCEPTABLE, "Not Acceptable");
    }
    let Some(conn_id) = header(&req, HEADER_CONNECTION_ID) else {
        return text(StatusCode::BAD_REQUEST, "Missing Acp-Connection-Id");
    };
    let conn = {
        let g = state.connections.lock().expect("conns");
        g.get(conn_id).cloned()
    };
    let Some(conn) = conn else {
        return text(StatusCode::NOT_FOUND, "Unknown Acp-Connection-Id");
    };
    if !same_principal(&state, &conn, &req) {
        return text(StatusCode::FORBIDDEN, "Forbidden");
    }
    let mailbox = match header(&req, HEADER_SESSION_ID) {
        Some(sid) => conn.ensure_session(sid),
        None => conn.connection_stream.clone(),
    };
    let Some(lease) = mailbox.try_acquire() else {
        return text(
            StatusCode::CONFLICT,
            "Outbound stream already has an active receiver",
        );
    };
    sse(lease)
}

fn delete(state: Arc<HttpState>, req: Request<Incoming>) -> Response<BoxBody<Bytes, Infallible>> {
    let Some(conn_id) = header(&req, HEADER_CONNECTION_ID) else {
        return text(StatusCode::BAD_REQUEST, "Missing Acp-Connection-Id");
    };
    let conn = {
        let mut g = state.connections.lock().expect("conns");
        match g.get(conn_id) {
            Some(c) if !same_principal(&state, c, &req) => {
                return text(StatusCode::FORBIDDEN, "Forbidden");
            }
            _ => g.remove(conn_id),
        }
    };
    let Some(conn) = conn else {
        return text(StatusCode::NOT_FOUND, "Unknown Acp-Connection-Id");
    };
    conn.shutdown();
    empty(StatusCode::ACCEPTED)
}

async fn initialize(
    state: Arc<HttpState>,
    message: Value,
    bearer: Option<String>,
) -> Response<BoxBody<Bytes, Infallible>> {
    let id = message.get("id").cloned();
    if id.as_ref().is_none_or(|v| v.is_null()) {
        return text(
            StatusCode::BAD_REQUEST,
            "Initialize request must include an ID",
        );
    }
    let (to_agent_tx, to_agent_rx) = futures::channel::mpsc::unbounded::<String>();
    let (from_agent_tx, mut from_agent_rx) = futures::channel::mpsc::unbounded::<String>();
    let incoming =
        to_agent_rx.map(Ok::<_, std::io::Error> as fn(String) -> std::io::Result<String>);
    let outgoing = from_agent_tx.sink_map_err(broken_pipe as fn(_) -> std::io::Error);
    let conn_id = uuid::Uuid::new_v4().to_string();
    let conn = Arc::new(Conn {
        bearer: bearer.clone(),
        to_agent: to_agent_tx,
        connection_stream: Mailbox::new(),
        sessions: Mutex::new(HashMap::new()),
        pending_routes: Mutex::new(HashMap::new()),
        client_response_routes: Mutex::new(HashMap::new()),
    });
    {
        let mut g = state.connections.lock().expect("conns");
        g.insert(conn_id.clone(), conn.clone());
    }
    let serving = (state.serve.connect)(
        bearer,
        agent_client_protocol::Lines::new(outgoing, incoming),
    );
    let agent_conn = conn.clone();
    tokio::spawn(async move {
        serving.await;
        conn_finish(&agent_conn);
    });
    if conn.to_agent.unbounded_send(message.to_string()).is_err() {
        remove_conn(&state, &conn_id);
        return json_rpc_init_error(id, "Initialize failed");
    }
    let line = match from_agent_rx.next().await {
        Some(s) => s,
        None => {
            remove_conn(&state, &conn_id);
            return json_rpc_init_error(id, "Initialize failed");
        }
    };
    let response: Value = match serde_json::from_str(&line) {
        Ok(v) => v,
        Err(_) => {
            remove_conn(&state, &conn_id);
            return json_rpc_init_error(id, "Initialize failed");
        }
    };
    if !is_matching_response(&response, id.as_ref().unwrap()) {
        remove_conn(&state, &conn_id);
        return json_rpc_init_error(id, "Initialize failed");
    }
    let router_conn = {
        let g = state.connections.lock().expect("conns");
        g.get(&conn_id).cloned()
    };
    if let Some(c) = router_conn {
        tokio::spawn(route_outbound(c, from_agent_rx));
    }
    json_with_conn(StatusCode::OK, response, &conn_id)
}

fn broken_pipe(e: futures::channel::mpsc::SendError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::BrokenPipe, e)
}

fn conn_finish(conn: &Conn) {
    conn.connection_stream.finish();
    let sessions: Vec<Mailbox> = conn
        .sessions
        .lock()
        .expect("sessions")
        .values()
        .cloned()
        .collect();
    for s in sessions {
        s.finish();
    }
}

fn remove_conn(state: &HttpState, id: &str) {
    if let Some(c) = state.connections.lock().expect("conns").remove(id) {
        c.shutdown();
    }
}

impl Conn {
    fn ensure_session(&self, sid: &str) -> Mailbox {
        let mut g = self.sessions.lock().expect("sessions");
        g.entry(sid.to_string())
            .or_insert_with(Mailbox::new)
            .clone()
    }

    fn shutdown(&self) {
        self.to_agent.close_channel();
        self.connection_stream.abort();
        let sessions: Vec<Mailbox> = self
            .sessions
            .lock()
            .expect("sessions")
            .values()
            .cloned()
            .collect();
        for s in sessions {
            s.abort();
        }
    }
}

fn forward(
    conn: &Conn,
    message: Value,
    sess_hdr: Option<&str>,
) -> Result<(), (StatusCode, &'static str)> {
    let obj = message.as_object().expect("object");
    if is_response(obj) {
        return forward_client_response(conn, &message, sess_hdr);
    }
    let route = determine_route(obj, sess_hdr)?;
    if let Route::Session(sid) = &route {
        conn.ensure_session(sid);
    }
    if let Some(key) = obj.get("id").and_then(message_id_key) {
        let pending = if method_of(obj) == Some("session/load") {
            Route::Connection
        } else {
            route
        };
        conn.pending_routes
            .lock()
            .expect("routes")
            .insert(key, pending);
    }
    conn.to_agent
        .unbounded_send(message.to_string())
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "connection closed"))?;
    Ok(())
}

fn forward_client_response(
    conn: &Conn,
    message: &Value,
    sess_hdr: Option<&str>,
) -> Result<(), (StatusCode, &'static str)> {
    let key = message.get("id").and_then(message_id_key);
    let route = key.as_ref().and_then(|k| {
        conn.client_response_routes
            .lock()
            .expect("routes")
            .get(k)
            .cloned()
    });
    if let Some(Route::Session(sid)) = &route {
        if sess_hdr.is_none() {
            return Err((StatusCode::BAD_REQUEST, "Missing Acp-Session-Id"));
        }
        if sess_hdr != Some(sid.as_str()) {
            return Err((StatusCode::BAD_REQUEST, "Mismatched Acp-Session-Id"));
        }
    }
    if let Some(k) = key {
        conn.client_response_routes
            .lock()
            .expect("routes")
            .remove(&k);
    }
    conn.to_agent
        .unbounded_send(message.to_string())
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "connection closed"))?;
    Ok(())
}

fn determine_route(
    obj: &Map<String, Value>,
    sess_hdr: Option<&str>,
) -> Result<Route, (StatusCode, &'static str)> {
    let params_sid = session_id_from_params(obj.get("params"));
    let method = method_of(obj);
    if (method.is_some_and(method_requires_session) || params_sid.is_some()) && sess_hdr.is_none() {
        return Err((StatusCode::BAD_REQUEST, "Missing Acp-Session-Id"));
    }
    if let (Some(h), Some(p)) = (sess_hdr, params_sid.as_deref())
        && h != p
    {
        return Err((StatusCode::BAD_REQUEST, "Mismatched Acp-Session-Id"));
    }
    if let Some(h) = sess_hdr {
        return Ok(Route::Session(h.to_string()));
    }
    if let Some(p) = params_sid {
        return Ok(Route::Session(p));
    }
    Ok(Route::Connection)
}

async fn route_outbound(
    conn: Arc<Conn>,
    mut rx: futures::channel::mpsc::UnboundedReceiver<String>,
) {
    while let Some(line) = rx.next().await {
        if let Ok(msg) = serde_json::from_str::<Value>(&line) {
            route_one(&conn, msg);
        }
    }
    conn.connection_stream.finish();
    let sessions: Vec<Mailbox> = conn
        .sessions
        .lock()
        .expect("sessions")
        .values()
        .cloned()
        .collect();
    for s in sessions {
        s.finish();
    }
}

fn route_one(conn: &Conn, message: Value) {
    let Some(obj) = message.as_object() else {
        conn.connection_stream.push(message);
        return;
    };
    if is_response(obj) {
        let key = obj.get("id").and_then(message_id_key);
        let route = key
            .as_ref()
            .and_then(|k| conn.pending_routes.lock().expect("routes").remove(k));
        if let Some(sid) = session_id_from_params(obj.get("result")) {
            conn.ensure_session(&sid);
        }
        push_route(conn, route.unwrap_or(Route::Connection), message);
        return;
    }
    if let Some(sid) = session_id_from_params(obj.get("params")) {
        if let Some(key) = obj.get("id").and_then(message_id_key) {
            conn.client_response_routes
                .lock()
                .expect("routes")
                .insert(key, Route::Session(sid.clone()));
        }
        conn.ensure_session(&sid).push(message);
        return;
    }
    if let Some(key) = obj.get("id").and_then(message_id_key) {
        conn.client_response_routes
            .lock()
            .expect("routes")
            .insert(key, Route::Connection);
    }
    conn.connection_stream.push(message);
}

fn push_route(conn: &Conn, route: Route, message: Value) {
    match route {
        Route::Connection => conn.connection_stream.push(message),
        Route::Session(sid) => conn.ensure_session(&sid).push(message),
    }
}

fn is_initialize(obj: &Map<String, Value>) -> bool {
    obj.get("jsonrpc").and_then(Value::as_str) == Some("2.0")
        && obj.contains_key("id")
        && method_of(obj) == Some("initialize")
}

fn is_response(obj: &Map<String, Value>) -> bool {
    obj.contains_key("id") && !obj.contains_key("method")
}

fn is_matching_response(msg: &Value, id: &Value) -> bool {
    msg.get("id") == Some(id) && msg.get("method").is_none()
}

fn method_of(obj: &Map<String, Value>) -> Option<&str> {
    obj.get("method").and_then(Value::as_str)
}

fn method_requires_session(method: &str) -> bool {
    SESSION_SCOPED.contains(&method)
}

fn session_id_from_params(params: Option<&Value>) -> Option<String> {
    params
        .and_then(Value::as_object)
        .and_then(|o| o.get("sessionId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn message_id_key(id: &Value) -> Option<String> {
    match id {
        Value::String(s) => Some(format!("string:{s}")),
        Value::Number(n) => Some(format!("number:{n}")),
        Value::Null => Some("null".into()),
        _ => None,
    }
}

fn text(status: StatusCode, body: &'static str) -> Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from_static(body.as_bytes())).boxed())
        .expect("response")
}

fn empty(status: StatusCode) -> Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(status)
        .body(Full::new(Bytes::new()).boxed())
        .expect("response")
}

fn json_with_conn(
    status: StatusCode,
    value: Value,
    conn_id: &str,
) -> Response<BoxBody<Bytes, Infallible>> {
    let body = serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
    Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, JSON_MIME)
        .header(
            HeaderName::from_static("acp-connection-id"),
            HeaderValue::from_str(conn_id).unwrap_or_else(|_| HeaderValue::from_static("")),
        )
        .body(Full::new(Bytes::from(body)).boxed())
        .expect("response")
}

fn json_rpc_init_error(id: Option<Value>, data: &str) -> Response<BoxBody<Bytes, Infallible>> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32603, "message": "Initialize failed", "data": data }
    });
    let bytes = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(hyper::header::CONTENT_TYPE, JSON_MIME)
        .body(Full::new(Bytes::from(bytes)).boxed())
        .expect("response")
}

fn sse(lease: Lease) -> Response<BoxBody<Bytes, Infallible>> {
    let (tx, rx) = mpsc::unbounded_channel::<Result<Frame<Bytes>, Infallible>>();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        interval.tick().await;
        loop {
            tokio::select! {
                biased;
                msg = lease.mailbox.recv(&lease) => {
                    match msg {
                        Some(v) => {
                            let payload = format!("data: {}\n\n", v);
                            if tx.send(Ok(Frame::data(Bytes::from(payload)))).is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                _ = interval.tick() => {
                    if tx.send(Ok(Frame::data(Bytes::from_static(b":\n\n")))).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let stream = UnboundedFrames { rx };
    Response::builder()
        .status(StatusCode::OK)
        .header(hyper::header::CONTENT_TYPE, EVENT_STREAM)
        .header(hyper::header::CACHE_CONTROL, "no-cache")
        .header(hyper::header::CONNECTION, "keep-alive")
        .body(BodyExt::boxed(StreamBody::new(stream)))
        .expect("response")
}

struct BoundedFrames {
    rx: mpsc::Receiver<Result<Frame<Bytes>, Infallible>>,
}

impl Stream for BoundedFrames {
    type Item = Result<Frame<Bytes>, Infallible>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

struct UnboundedFrames {
    rx: mpsc::UnboundedReceiver<Result<Frame<Bytes>, Infallible>>,
}

impl Stream for UnboundedFrames {
    type Item = Result<Frame<Bytes>, Infallible>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

fn resolve_addr(addr: &str) -> Result<SocketAddr, String> {
    if let Ok(s) = addr.parse() {
        return Ok(s);
    }
    std::net::ToSocketAddrs::to_socket_addrs(addr)
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| format!("bad --acp-http address {addr}"))
}
