use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::net::SocketAddr;
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
const MAX_BODY: usize = 16 * 1024 * 1024;

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

pub(crate) async fn listen(process: Args, addr: String) -> Result<(), String> {
    let bind = resolve_addr(&addr)?;
    let listener = TcpListener::bind(bind)
        .await
        .map_err(|e| format!("bind {bind}: {e}"))?;
    let local = listener.local_addr().map_err(|e| e.to_string())?;
    eprintln!("rung-agent: ACP HTTP at http://{local}/acp");
    let state = Arc::new(HttpState {
        process: Arc::new(process.clone()),
        live: Live::default(),
        token: process.acp_token.clone(),
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
    process: Arc<Args>,
    live: Live,
    token: Option<String>,
    connections: Mutex<HashMap<String, Arc<Conn>>>,
}

struct Conn {
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

fn authorized(state: &HttpState, req: &Request<Incoming>) -> bool {
    let Some(token) = state.token.as_deref() else {
        return true;
    };
    header(req, "authorization") == Some(&format!("Bearer {token}"))
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
        return text(StatusCode::NOT_FOUND, "Not Found");
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

async fn post(
    state: Arc<HttpState>,
    req: Request<Incoming>,
) -> Response<BoxBody<Bytes, Infallible>> {
    if !is_json_content_type(&req) {
        return text(StatusCode::UNSUPPORTED_MEDIA_TYPE, "Unsupported Media Type");
    }
    let conn_hdr = header(&req, HEADER_CONNECTION_ID).map(str::to_string);
    let sess_hdr = header(&req, HEADER_SESSION_ID).map(str::to_string);
    let collected = match req.collect().await {
        Ok(c) => c.to_bytes(),
        Err(_) => return text(StatusCode::BAD_REQUEST, "Invalid JSON"),
    };
    if collected.len() > MAX_BODY {
        return text(StatusCode::PAYLOAD_TOO_LARGE, "Request body too large");
    }
    let value: Value = match serde_json::from_slice(&collected) {
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
        return initialize(state, value).await;
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
        g.remove(conn_id)
    };
    let Some(conn) = conn else {
        return text(StatusCode::NOT_FOUND, "Unknown Acp-Connection-Id");
    };
    conn.shutdown();
    empty(StatusCode::ACCEPTED)
}

async fn initialize(state: Arc<HttpState>, message: Value) -> Response<BoxBody<Bytes, Infallible>> {
    let id = message.get("id").cloned();
    if id.as_ref().is_none_or(|v| v.is_null()) {
        return text(
            StatusCode::BAD_REQUEST,
            "Initialize request must include an ID",
        );
    }
    let (to_agent_tx, to_agent_rx) = futures::channel::mpsc::unbounded::<String>();
    let (from_agent_tx, mut from_agent_rx) = futures::channel::mpsc::unbounded::<String>();
    let incoming = to_agent_rx.map(Ok::<_, std::io::Error>);
    let outgoing =
        from_agent_tx.sink_map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e));
    let conn_id = uuid::Uuid::new_v4().to_string();
    let conn = Arc::new(Conn {
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
    let process = state.process.clone();
    let live = state.live.clone();
    let agent_conn = conn.clone();
    tokio::spawn(async move {
        let _ = connect_agent(
            process,
            live,
            agent_client_protocol::Lines::new(outgoing, incoming),
        )
        .await;
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
