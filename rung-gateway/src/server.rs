//! The listener and the routes: `/api/health`, `/api/instances`,
//! `/api/i/{id}/v1/...` (the pass-through), and the app for everything else.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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

use crate::config::Settings;
use crate::doors::{self, Access};
use crate::registry::{Instance, Registry, StaticRegistry};
use crate::role::{self, Role, Who};

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

struct State {
    read_only_tokens: Vec<String>,
    app_dir: Option<PathBuf>,
    registry: Arc<dyn Registry>,
    client: Client<HttpConnector, Body>,
}

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
    let listener = tokio::net::TcpListener::bind(settings.listen)
        .await
        .map_err(|e| format!("bind {}: {e}", settings.listen))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let mut connector = HttpConnector::new();
    connector.set_connect_timeout(Some(CONNECT_WAIT));
    let state = Arc::new(State {
        read_only_tokens: settings.read_only_tokens,
        app_dir: settings.app_dir,
        registry,
        client: Client::builder(TokioExecutor::new()).build(connector),
    });
    let task = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
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

async fn instances(state: &State) -> Response<Body> {
    let list = state.registry.list();
    let rows = futures::future::join_all(list.iter().map(|i| summary_row(state, i))).await;
    json_reply(StatusCode::OK, json!({ "instances": rows }))
}

async fn summary_row(state: &State, inst: &Instance) -> Value {
    let probe = async {
        let req = Request::builder()
            .uri(format!("{}/v1/summary", inst.url))
            .header("authorization", format!("Bearer {}", inst.key))
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
    let Some(inst) = state.registry.get(id) else {
        return error(
            StatusCode::NOT_FOUND,
            "no_such_instance",
            "no instance has that id",
        );
    };
    // only the host's own /v1 doors pass; its ACP and anything else do not
    if !rest.starts_with("/v1/") || !plain_segments(rest) {
        return not_found();
    }
    if role == Role::ReadOnly && doors::classify(req.method().as_str(), rest) == Access::Write {
        return read_only();
    }
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
    up.headers_mut().insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(&format!("Bearer {}", inst.key)).unwrap(),
    );
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
