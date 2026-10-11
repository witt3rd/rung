//! The listener and the routes: `/api/health`, `/api/instances`,
//! `/api/i/{id}/v1/...` (the pass-through), and the app for everything else.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header::{HeaderMap, HeaderName, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use serde_json::{Value, json};
use tokio::sync::mpsc::error::SendTimeoutError;

use crate::config::Settings;
use crate::doors::{self, Access};
use crate::registry::{Instance, Registry, StaticRegistry};
use crate::role::{self, Role, Who};
use rung_host::redact::Redactor;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;
type Body = BoxBody<Bytes, BoxErr>;

/// How long the overview waits for one instance's summary before listing it
/// as unreachable. A bound on a probe, not on any content.
const SUMMARY_WAIT: Duration = Duration::from_secs(5);
const CONNECT_WAIT: Duration = Duration::from_secs(5);

/// A running gateway.
pub struct Running {
    pub addr: SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl Running {
    /// Stop accepting; connections already open run out.
    pub fn shutdown(self) {
        self.task.abort();
    }
}

/// How long the listener rests after a failed `accept` before it tries again.
const ACCEPT_PAUSE: Duration = Duration::from_millis(100);

/// Wait for the next connection. A failed accept (an exhausted descriptor
/// table, say) is reported and retried after `pause`, never spun on.
async fn accept_retrying<T, E, Fut>(
    mut accept: impl FnMut() -> Fut,
    pause: Duration,
    mut report: impl FnMut(&E),
) -> T
where
    Fut: std::future::Future<Output = Result<T, E>>,
{
    loop {
        match accept().await {
            Ok(c) => return c,
            Err(e) => {
                report(&e);
                tokio::time::sleep(pause).await;
            }
        }
    }
}

struct State {
    bound_host: Option<String>,
    allowed_hosts: Vec<String>,
    read_only_tokens: Vec<String>,
    app_dir: Option<PathBuf>,
    registry: Arc<dyn Registry>,
    /// The registry folder of native hosts; their H3 doors are read from
    /// each one's own state directory.
    native_dir: Option<PathBuf>,
    /// The gateway's own secrets: read-only tokens and instance keys. The
    /// doors that read a state directory redact these, and what each
    /// instance's config names, from everything they return.
    redactor: Redactor,
    client: Client<HttpConnector, Body>,
    tuning: Tuning,
}

/// Knobs of a running gateway that tests turn; the defaults are what runs.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    /// How long a page being streamed waits for its client to take the next
    /// chunk before the client is dropped. A bound on a stall, never on the
    /// size of a page or how long a client that keeps reading takes.
    pub stall_deadline: Duration,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            stall_deadline: STALL_DEADLINE,
        }
    }
}

/// A client that takes no chunk of a streamed page for this long is dropped.
const STALL_DEADLINE: Duration = Duration::from_secs(30);

/// Serve the settings' own instances.
pub async fn start(settings: Settings) -> Result<Running, String> {
    let reg = Arc::new(StaticRegistry(settings.instances.clone()));
    start_with_registry(settings, reg).await
}

/// Serve whatever instances `registry` lists, asked on every request.
pub async fn start_with_registry(
    settings: Settings,
    registry: Arc<dyn Registry>,
) -> Result<Running, String> {
    start_tuned(settings, registry, Tuning::default()).await
}

/// As [`start_with_registry`], with the knobs set.
pub async fn start_tuned(
    settings: Settings,
    registry: Arc<dyn Registry>,
    tuning: Tuning,
) -> Result<Running, String> {
    let listener = tokio::net::TcpListener::bind(settings.listen)
        .await
        .map_err(|e| format!("bind {}: {e}", settings.listen))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let mut connector = HttpConnector::new();
    connector.set_connect_timeout(Some(CONNECT_WAIT));
    let mut redactor = Redactor::new();
    for t in &settings.read_only_tokens {
        redactor.add_secret(t);
    }
    for i in &settings.instances {
        redactor.add_secret(&i.key);
    }
    let state = Arc::new(State {
        bound_host: bound_host(addr.ip()),
        allowed_hosts: settings.allowed_hosts,
        read_only_tokens: settings.read_only_tokens,
        app_dir: settings.app_dir,
        registry,
        native_dir: settings.registry_dir,
        redactor,
        client: Client::builder(TokioExecutor::new()).build(connector),
        tuning,
    });
    let task = tokio::spawn(async move {
        loop {
            let (stream, _) = accept_retrying(
                || listener.accept(),
                ACCEPT_PAUSE,
                |e| eprintln!("rung-gateway: accept failed: {e}"),
            )
            .await;
            let state = state.clone();
            tokio::spawn(async move {
                let svc = hyper::service::service_fn(move |req| {
                    let state = state.clone();
                    async move { Ok::<_, std::convert::Infallible>(handle(state, req).await) }
                });
                let _ = AutoBuilder::new(TokioExecutor::new())
                    .serve_connection(TokioIo::new(stream), svc)
                    .await;
            });
        }
    });
    Ok(Running { addr, task })
}

fn bound_host(ip: std::net::IpAddr) -> Option<String> {
    if ip.is_loopback() || ip.is_unspecified() {
        return None;
    }
    Some(match ip {
        std::net::IpAddr::V4(v) => v.to_string(),
        std::net::IpAddr::V6(v) => format!("[{v}]"),
    })
}

fn full(status: StatusCode, content_type: &str, body: impl Into<Bytes>) -> Response<Body> {
    let mut r = Response::new(Full::new(body.into()).map_err(|n| match n {}).boxed());
    *r.status_mut() = status;
    r.headers_mut()
        .insert("content-type", HeaderValue::from_str(content_type).unwrap());
    r
}

fn json_reply(status: StatusCode, v: Value) -> Response<Body> {
    full(status, "application/json", v.to_string())
}

fn error(status: StatusCode, code: &str, message: &str) -> Response<Body> {
    json_reply(status, json!({ "error": code, "message": message }))
}

async fn handle(state: Arc<State>, req: Request<Incoming>) -> Response<Body> {
    let path = req.uri().path().to_string();
    if path == "/api" || path.starts_with("/api/") {
        api(&state, req, &path).await
    } else {
        app(&state, req.method(), &path)
    }
}

/// The query split into what is forwarded and the `token` value (which is
/// the gateway's own and never goes upstream).
fn split_query(query: Option<&str>) -> (Option<String>, Option<String>) {
    let mut token = None;
    let mut keep = Vec::new();
    for pair in query.unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        match pair.split_once('=') {
            Some(("token", v)) => token = Some(decode(v).unwrap_or_default()),
            None if pair == "token" => token = Some(String::new()),
            _ => keep.push(pair),
        }
    }
    let rest = (!keep.is_empty()).then(|| keep.join("&"));
    (rest, token)
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    let v = headers.get("authorization")?.to_str().ok()?;
    let (scheme, token) = v.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| token.trim().to_string())
}

async fn api(state: &State, req: Request<Incoming>, path: &str) -> Response<Body> {
    let (query, query_token) = split_query(req.uri().query());
    let role = match role::resolve(
        bearer(req.headers()).as_deref(),
        query_token.as_deref(),
        &state.read_only_tokens,
    ) {
        Who::Known(r) => r,
        Who::Unknown => {
            return error(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "the token is not recognised",
            );
        }
    };
    let method = req.method().clone();
    let reading = matches!(method, Method::GET | Method::HEAD);
    if role == Role::Owner
        && let Some(refusal) = owner_guard(state, &req, !reading)
    {
        return refusal;
    }
    match path {
        "/api/health" if reading => json_reply(StatusCode::OK, json!({ "ok": true })),
        "/api/instances" if reading => instances(state).await,
        "/api/health" | "/api/instances" => {
            if role == Role::ReadOnly {
                read_only()
            } else {
                error(
                    StatusCode::METHOD_NOT_ALLOWED,
                    "method_not_allowed",
                    "this door only reads",
                )
            }
        }
        _ => match path.strip_prefix("/api/i/") {
            Some(tail) => pass_through(state, req, role, tail, query).await,
            None => not_found(),
        },
    }
}

/// The owner role is whoever reaches the gateway, so a write must not be
/// drivable, nor a read leaked, by a rebound DNS name: the `Host` of every
/// owner request must be loopback, the bound address or a configured name. A
/// write also needs an `Origin`, if the client sends one (a browser always
/// does on a write), that is this same origin. A script sends no `Origin`
/// and passes.
fn owner_guard(state: &State, req: &Request<Incoming>, write: bool) -> Option<Response<Body>> {
    let bad_host = || {
        Some(error(
            StatusCode::FORBIDDEN,
            "bad_host",
            "this host name is not one the gateway answers for",
        ))
    };
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .or_else(|| req.uri().authority().map(|a| a.as_str()));
    let Some(host) = host else { return bad_host() };
    let (name, host_port) = split_authority(host);
    let known = matches!(name.as_str(), "localhost" | "127.0.0.1" | "[::1]")
        || state.bound_host.as_deref() == Some(name.as_str())
        || state.allowed_hosts.contains(&name);
    if !known {
        return bad_host();
    }
    if !write {
        return None;
    }
    let origin = req.headers().get("origin")?;
    let same = origin.to_str().ok().is_some_and(|o| {
        let Some((scheme, rest)) = o.split_once("://") else {
            return false;
        };
        let default_port = match scheme.to_ascii_lowercase().as_str() {
            "http" => 80,
            "https" => 443,
            _ => return false,
        };
        let (oname, oport) = split_authority(rest.split('/').next().unwrap_or(""));
        oname == name && oport.unwrap_or(default_port) == host_port.unwrap_or(default_port)
    });
    (!same).then(|| {
        error(
            StatusCode::FORBIDDEN,
            "bad_origin",
            "a write must come from the gateway's own origin",
        )
    })
}

/// `host[:port]` as a lower-case name and the explicit port, if any.
fn split_authority(a: &str) -> (String, Option<u16>) {
    let a = a.to_ascii_lowercase();
    let cut = if a.starts_with('[') {
        a.find(']')
            .and_then(|i| a[i + 1..].starts_with(':').then_some(i + 1))
    } else {
        a.rfind(':')
    };
    match cut {
        Some(i) => (a[..i].to_string(), a[i + 1..].parse().ok()),
        None => (a, None),
    }
}

fn not_found() -> Response<Body> {
    error(StatusCode::NOT_FOUND, "not_found", "no such door")
}

fn read_only() -> Response<Body> {
    error(
        StatusCode::FORBIDDEN,
        "read_only",
        "this token can only read",
    )
}

/// A redactor for one request: the gateway's secrets, and the key of every
/// instance the registry lists now.
fn request_redactor(state: &State) -> Redactor {
    let mut r = state.redactor.clone();
    for i in state.registry.list() {
        r.add_secret(&i.key);
    }
    r
}

/// Run a blocking read of state directories off the async threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(f).await.ok()
}

async fn instances(state: &State) -> Response<Body> {
    // native instances: read from their state directories, running or not
    let (native, registry_error) = match &state.native_dir {
        None => (Vec::new(), Value::Null),
        Some(dir) => {
            let (dir, red) = (dir.clone(), request_redactor(state));
            match blocking(move || rung_host::read::rows(&dir, &red)).await {
                Some(Ok(rows)) => (rows, Value::Null),
                Some(Err(e)) => (Vec::new(), json!(e.message)),
                None => (Vec::new(), json!("the registry could not be read")),
            }
        }
    };
    // one row per id: a configured instance the registry also lists is the
    // registry's row
    let list: Vec<Instance> = state
        .registry
        .list()
        .into_iter()
        .filter(|i| !native.iter().any(|n| n["id"] == i.id.as_str()))
        .collect();
    let mut rows = futures::future::join_all(list.iter().map(|i| summary_row(state, i))).await;
    rows.extend(native);
    json_reply(
        StatusCode::OK,
        json!({ "instances": rows, "registry_error": registry_error }),
    )
}

async fn summary_row(state: &State, inst: &Instance) -> Value {
    let probe = async {
        let authorization = inst
            .authorization()
            .ok_or("the instance's key cannot be sent as a header")?;
        let req = Request::builder()
            .uri(format!("{}/v1/summary", inst.url))
            .header("authorization", authorization)
            .body(empty())
            .map_err(|e| e.to_string())?;
        let resp = state.client.request(req).await.map_err(|e| chain(&e))?;
        let status = resp.status();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| e.to_string())?
            .to_bytes();
        if !status.is_success() {
            return Err(format!("the instance answered {status}"));
        }
        serde_json::from_slice::<Value>(&bytes).map_err(|_| "the summary is not JSON".to_string())
    };
    let outcome = match tokio::time::timeout(SUMMARY_WAIT, probe).await {
        Ok(r) => r,
        Err(_) => Err("the instance did not answer in time".into()),
    };
    let (reachable, summary, err) = match outcome {
        Ok(v) => (true, v, Value::Null),
        Err(e) => (false, Value::Null, Value::String(scrub(&e, inst))),
    };
    json!({
        "id": inst.id,
        "name": inst.name,
        "source": "config",
        "reachable": reachable,
        "summary": summary,
        "error": err,
    })
}

/// An error with its causes, in one line.
fn chain(e: &dyn std::error::Error) -> String {
    let mut out = e.to_string();
    let mut src = e.source();
    while let Some(s) = src {
        out.push_str(": ");
        out.push_str(&s.to_string());
        src = s.source();
    }
    out
}

/// An error text never carries the instance's key.
fn scrub(text: &str, inst: &Instance) -> String {
    text.replace(&inst.key, "[key]")
}

fn empty() -> Body {
    Full::new(Bytes::new()).map_err(|n| match n {}).boxed()
}

/// Percent-decode; `None` if the bytes are not UTF-8.
fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && b.len() >= i + 3 {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// A path whose every segment, once decoded, is a plain name: no `.`, `..`,
/// separator or NUL.
fn plain_segments(path: &str) -> bool {
    path.split('/')
        .filter(|s| !s.is_empty())
        .all(|seg| match decode(seg) {
            Some(d) => d != "." && d != ".." && !d.contains(['/', '\\', '\0']),
            None => false,
        })
}

const HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Copy end-to-end headers, minus those named `drop`.
fn forward_headers(from: &HeaderMap, to: &mut HeaderMap, drop: &[&str]) {
    let named: Vec<String> = from
        .get_all("connection")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(|t| t.trim().to_ascii_lowercase())
        .collect();
    for (k, v) in from {
        let n = k.as_str();
        if HOP.contains(&n) || drop.contains(&n) || named.iter().any(|x| x == n) {
            continue;
        }
        to.append(k.clone(), v.clone());
    }
}

async fn pass_through(
    state: &State,
    req: Request<Incoming>,
    role: Role,
    tail: &str,
    query: Option<String>,
) -> Response<Body> {
    let (id, rest) = match tail.find('/') {
        Some(i) => (&tail[..i], &tail[i..]),
        None => (tail, ""),
    };
    let native = state
        .native_dir
        .as_ref()
        .filter(|d| rung_host::read::is_registered(d, id));
    let configured = state.registry.get(id);
    if native.is_none() && configured.is_none() {
        return error(
            StatusCode::NOT_FOUND,
            "no_such_instance",
            "no instance has that id",
        );
    }
    // only the host's own /v1 doors pass; its ACP and anything else do not
    if !rest.starts_with("/v1/") || !plain_segments(rest) {
        return not_found();
    }
    if role == Role::ReadOnly && doors::classify(req.method().as_str(), rest) == Access::Write {
        return read_only();
    }
    if let Some(dir) = native
        && rest == "/v1/events"
    {
        return native_events(state, dir.clone(), id, &req, query).await;
    }
    if let Some(dir) = native
        && matches!(rest, "/v1/summary" | "/v1/record")
    {
        return native_door(state, dir.clone(), id, rest, &req, query).await;
    }
    // a native instance has no listener the gateway knows: only the doors
    // read from its state directory exist for it until a later slice adds more
    let Some(inst) = configured else {
        return not_found();
    };
    let Some(authorization) = inst.authorization() else {
        return error(
            StatusCode::BAD_GATEWAY,
            "instance_unreachable",
            "the instance's key cannot be sent as a header",
        );
    };
    let target = match &query {
        Some(q) => format!("{}{rest}?{q}", inst.url),
        None => format!("{}{rest}", inst.url),
    };
    let (parts, body) = req.into_parts();
    let mut up = Request::new(body.map_err(|e| Box::new(e) as BoxErr).boxed());
    *up.method_mut() = parts.method;
    *up.uri_mut() = match target.parse() {
        Ok(u) => u,
        Err(_) => return not_found(),
    };
    forward_headers(
        &parts.headers,
        up.headers_mut(),
        &["host", "authorization", "cookie"],
    );
    up.headers_mut()
        .insert(HeaderName::from_static("authorization"), authorization);
    match state.client.request(up).await {
        Ok(resp) => {
            let (rp, rb) = resp.into_parts();
            let mut out = Response::new(rb.map_err(|e| Box::new(e) as BoxErr).boxed());
            *out.status_mut() = rp.status;
            forward_headers(&rp.headers, out.headers_mut(), &[]);
            out
        }
        Err(e) => error(
            StatusCode::BAD_GATEWAY,
            "instance_unreachable",
            &scrub(&chain(&e), &inst),
        ),
    }
}

/// The query's pairs, decoded; `None` when a value is not valid UTF-8.
fn query_pairs(query: Option<&str>) -> Option<Vec<(String, String)>> {
    rung_host::stream::query_pairs(query)
}

/// An H3 read door of a native instance, answered from its state directory.
/// The summary is one small object; the record's page is streamed.
async fn native_door(
    state: &State,
    dir: PathBuf,
    id: &str,
    door: &str,
    req: &Request<Incoming>,
    query: Option<String>,
) -> Response<Body> {
    if !matches!(*req.method(), Method::GET | Method::HEAD) {
        return error(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "this door only reads",
        );
    }
    let red = request_redactor(state);
    let id = id.to_string();
    if door == "/v1/summary" {
        if query.as_deref().is_some_and(|q| !q.is_empty()) {
            return error(
                StatusCode::BAD_REQUEST,
                "bad_request",
                "this door takes no parameters",
            );
        }
        let r = red.clone();
        return match blocking(move || rung_host::read::summary(&dir, &id, &r)).await {
            Some(Ok(v)) => json_reply(StatusCode::OK, v),
            Some(Err(e)) => door_error(e, &red),
            None => internal(),
        };
    }
    let Some(pairs) = query_pairs(query.as_deref()) else {
        return error(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "the query is not valid UTF-8",
        );
    };
    let q = match rung_host::read::RecordQuery::parse(&pairs) {
        Ok(q) => q,
        Err(e) => return door_error(e, &red),
    };
    let r = red.clone();
    match blocking(move || rung_host::read::record_open(&dir, &id, &q, &r)).await {
        Some(Ok(page)) => stream_page(page, state.tuning.stall_deadline),
        Some(Err(e)) => door_error(e, &red),
        None => internal(),
    }
}

fn internal() -> Response<Body> {
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal",
        "the read failed",
    )
}

/// Bytes written to the page are sent in chunks of about this size.
const CHUNK: usize = 64 * 1024;
/// Chunks waiting for the client: the writer blocks when it is this far ahead,
/// so a slow client holds back the read instead of the read filling memory.
const AHEAD: usize = 2;

/// A writer that hands what it is given to a channel in chunks.
struct ChannelWriter {
    tx: tokio::sync::mpsc::Sender<Result<Bytes, BoxErr>>,
    buf: Vec<u8>,
    /// The runtime, to wait on the channel from this blocking thread.
    rt: tokio::runtime::Handle,
    /// How long the client may take no chunk before it is dropped.
    stall: Duration,
    /// Set when the client was dropped for stalling, so the body ends in an
    /// error and not as a complete-looking short page.
    cut: Arc<AtomicBool>,
}

impl ChannelWriter {
    fn send(&mut self) -> std::io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let chunk = Bytes::from(std::mem::replace(&mut self.buf, Vec::with_capacity(CHUNK)));
        match self
            .rt
            .block_on(self.tx.send_timeout(Ok(chunk), self.stall))
        {
            Ok(()) => Ok(()),
            Err(SendTimeoutError::Timeout(_)) => {
                self.cut.store(true, Ordering::SeqCst);
                Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "client stalled",
                ))
            }
            Err(SendTimeoutError::Closed(_)) => Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "client gone",
            )),
        }
    }
}

impl std::io::Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(data);
        if self.buf.len() >= CHUNK {
            self.send()?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.send()
    }
}

/// Answer 200 and write the page as the reader produces it: one blocking task
/// reads and redacts a line at a time into a channel of a few chunks, and the
/// body is that channel. The task waits for a client that is slow, but only
/// while the client keeps taking chunks: one that takes none for `stall` is
/// dropped, which frees the task's blocking-pool slot (a pool of such waits
/// would starve every other request). A failure part-way (the record became
/// unreadable, or the client was dropped) ends the body with an error, which
/// closes the connection: the client sees a truncated answer, never a
/// complete-looking short one.
fn stream_page(page: rung_host::read::RecordPage, stall: Duration) -> Response<Body> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, BoxErr>>(AHEAD);
    let cut = Arc::new(AtomicBool::new(false));
    let (rt, flag) = (tokio::runtime::Handle::current(), cut.clone());
    tokio::task::spawn_blocking(move || {
        let mut w = ChannelWriter {
            tx: tx.clone(),
            buf: Vec::with_capacity(CHUNK),
            rt: rt.clone(),
            stall,
            cut: flag,
        };
        match page.write_json(&mut w) {
            Ok(()) => {}
            // the client went away: nobody is left to tell
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
            // unreadable part-way, or the client was dropped for stalling:
            // the channel may be full, so the failure is a flag the body
            // reads after its last chunk (see the stream below)
            Err(_) => w.cut.store(true, Ordering::SeqCst),
        }
    });
    // after the last chunk, a body whose writer failed ends in an error
    let stream = futures::stream::unfold((rx, cut, false), |(mut rx, st, done)| async move {
        if done {
            return None;
        }
        match rx.recv().await {
            Some(item) => Some((item, (rx, st, false))),
            None if st.load(Ordering::SeqCst) => {
                let e: BoxErr = "the page was cut short".into();
                Some((Err(e), (rx, st, true)))
            }
            None => None,
        }
    });
    let body = http_body_util::StreamBody::new(futures::StreamExt::map(stream, |item| {
        item.map(hyper::body::Frame::data)
    }))
    .boxed();
    let mut r = Response::new(body);
    r.headers_mut()
        .insert("content-type", HeaderValue::from_static("application/json"));
    r
}

/// How long the gateway waits for a running host's own door to answer before it
/// streams the record without partial frames. A bound on a probe, not on any
/// content.
const UPSTREAM_WAIT: Duration = Duration::from_secs(5);
/// Partial frames waiting between the host's door and one client.
const UPSTREAM_AHEAD: usize = 64;

/// `GET /v1/events` of a native instance (H5): the record from the client's
/// cursor, then as it is written, read from the instance's own state
/// directory by one async reader per client; a stopped instance replays and
/// ends. The partial frames of a running host's turn are relayed from the
/// host's own door when the registry gives its address and the gateway holds
/// its token. Every frame is redacted with the registry-wide redactor.
async fn native_events(
    state: &State,
    dir: PathBuf,
    id: &str,
    req: &Request<Incoming>,
    query: Option<String>,
) -> Response<Body> {
    use rung_host::stream::{Events, EventsQuery, Options, Partials, sse_headers};
    let head = *req.method() == Method::HEAD;
    if !matches!(*req.method(), Method::GET | Method::HEAD) {
        return error(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "this door only reads",
        );
    }
    let base = request_redactor(state);
    let bad = |m: &str| error(StatusCode::BAD_REQUEST, "bad_request", m);
    let Some(pairs) = query_pairs(query.as_deref()) else {
        return bad("the query is not UTF-8");
    };
    let last_id = req
        .headers()
        .get("last-event-id")
        .and_then(|v| v.to_str().ok());
    let q = match EventsQuery::parse(&pairs, last_id) {
        Ok(q) => q,
        Err(e) => return door_error(e, &base),
    };
    let (rdir, rid, rbase) = (dir, id.to_string(), base.clone());
    let target = match blocking(move || rung_host::read::stream_target(&rdir, &rid, &rbase)).await {
        Some(Ok(t)) => t,
        Some(Err(e)) => return door_error(e, &base),
        None => return internal(),
    };
    let upstream = match (&target.address, q.partials) {
        (Some(addr), Partials::On | Partials::Only) if !head => {
            host_partials(state, addr, &target.tokens).await
        }
        _ => None,
    };
    let mut o = Options::new(target.red.clone());
    o.after = q.after;
    o.partials = q.partials;
    o.env_unset = target.env_unset.clone();
    o.upstream = upstream;
    let events = match Events::open(&target.state_dir, o).await {
        Ok(ev) => ev,
        Err(e) => return door_error(e, &target.red),
    };
    let body = if head { empty() } else { sse_body(events) };
    let mut r = Response::new(body);
    for (k, v) in sse_headers() {
        if let (Ok(k), Ok(v)) = (HeaderName::try_from(k.as_str()), HeaderValue::from_str(&v)) {
            r.headers_mut().insert(k, v);
        }
    }
    r
}

/// A client's event stream as a response body. One task asks the stream for
/// its next frame only when the client has taken the last (a channel of one),
/// and it, with the stream's file and subscriptions, goes the moment the client
/// does. It holds no thread while it waits.
fn sse_body(mut events: rung_host::stream::Events) -> Body {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, BoxErr>>(1);
    tokio::spawn(async move {
        loop {
            let frame = tokio::select! {
                _ = tx.closed() => break,
                f = events.next() => f,
            };
            let Some(frame) = frame else { break };
            if tx.send(Ok(Bytes::from(frame.sse()))).await.is_err() {
                break;
            }
        }
    });
    let stream = futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    http_body_util::StreamBody::new(futures::StreamExt::map(stream, |item| {
        item.map(hyper::body::Frame::data)
    }))
    .boxed()
}

/// Ask a running host's own door for its partial frames (`partials=only`) and
/// relay them as they come. `None` when the host cannot be reached or takes no
/// token the gateway holds: the record still streams, and the first frame says
/// there are no partial frames. A host that goes away later ends the relay;
/// the record goes on.
async fn host_partials(
    state: &State,
    address: &str,
    tokens: &[String],
) -> Option<tokio::sync::mpsc::Receiver<rung_host::stream::Partial>> {
    let token = tokens.first()?;
    let authorization = HeaderValue::from_str(&format!("Bearer {token}")).ok()?;
    let req = Request::builder()
        .uri(format!("http://{address}/v1/events?partials=only"))
        .header("authorization", authorization)
        .header("accept", "text/event-stream")
        .body(empty())
        .ok()?;
    let resp = tokio::time::timeout(UPSTREAM_WAIT, state.client.request(req))
        .await
        .ok()?
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let (tx, rx) = tokio::sync::mpsc::channel(UPSTREAM_AHEAD);
    let mut body = resp.into_body();
    tokio::spawn(async move {
        let mut buf = String::new();
        loop {
            let frame = tokio::select! {
                _ = tx.closed() => break,
                f = body.frame() => f,
            };
            let Some(Ok(frame)) = frame else { break };
            let Some(data) = frame.data_ref() else {
                continue;
            };
            buf.push_str(&String::from_utf8_lossy(data));
            while let Some(i) = buf.find("\n\n") {
                let block: String = buf.drain(..i + 2).collect();
                if let Some(p) = parse_delta(&block)
                    && tx.send(p).await.is_err()
                {
                    return;
                }
            }
        }
    });
    Some(rx)
}

/// A `delta` frame of the host's door, as a partial frame; anything else
/// (comments, the open frame) is nothing to relay.
fn parse_delta(block: &str) -> Option<rung_host::stream::Partial> {
    let mut event = "";
    let mut data = String::new();
    for l in block.lines() {
        if let Some(v) = l.strip_prefix("event: ") {
            event = v;
        } else if let Some(v) = l.strip_prefix("data: ") {
            data.push_str(v);
        }
    }
    (event == "delta")
        .then(|| serde_json::from_str(&data).ok())
        .flatten()
}

fn door_error(e: rung_host::read::DoorError, red: &Redactor) -> Response<Body> {
    error(
        StatusCode::from_u16(e.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        e.code,
        &red.redact(&e.message),
    )
}

fn app(state: &State, method: &Method, path: &str) -> Response<Body> {
    if !matches!(*method, Method::GET | Method::HEAD) {
        return error(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "the app is read only",
        );
    }
    let Some(dir) = &state.app_dir else {
        return error(
            StatusCode::NOT_FOUND,
            "no_app",
            "the gateway was started without an app",
        );
    };
    if !plain_segments(path) {
        return not_found();
    }
    let rel: PathBuf = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| decode(s).unwrap_or_default())
        .collect();
    let file = dir.join(&rel);
    let served = if file.is_file() {
        Some(file)
    } else if !has_extension(path) {
        // a client-side route: the page itself
        Some(dir.join("index.html")).filter(|p| p.is_file())
    } else {
        None
    };
    match served.and_then(|f| std::fs::read(&f).ok().map(|b| (content_type(&f), b))) {
        Some((ct, bytes)) => full(StatusCode::OK, ct, bytes),
        None => not_found(),
    }
}

fn has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.'))
}

fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_a_specific_non_loopback_bind_is_an_allowed_host() {
        let b = |s: &str| super::bound_host(s.parse().unwrap());
        assert_eq!(b("100.64.0.9").as_deref(), Some("100.64.0.9"));
        assert_eq!(b("fd7a::1").as_deref(), Some("[fd7a::1]"));
        assert_eq!(b("127.0.0.1"), None);
        assert_eq!(b("::1"), None);
        assert_eq!(b("0.0.0.0"), None);
        assert_eq!(b("::"), None);
    }

    use super::*;
    use std::cell::Cell;

    /// A listener that keeps failing is reported on every failure and rests
    /// between tries: it does not spin.
    #[tokio::test]
    async fn a_failing_accept_is_reported_and_backed_off() {
        let calls = Cell::new(0u32);
        let reported = Cell::new(0u32);
        let t0 = std::time::Instant::now();
        let got = accept_retrying(
            || {
                calls.set(calls.get() + 1);
                let n = calls.get();
                async move {
                    if n <= 3 {
                        Err(format!("fd exhausted {n}"))
                    } else {
                        Ok(n)
                    }
                }
            },
            Duration::from_millis(40),
            |_| reported.set(reported.get() + 1),
        )
        .await;
        assert_eq!(got, 4);
        assert_eq!(reported.get(), 3, "every failure is reported");
        assert!(
            t0.elapsed() >= Duration::from_millis(120),
            "it rests between tries"
        );
    }
}
