//! The gateway's checks (design note H9): it serves the app, passes `/v1` and
//! streams through, adds the key, lists the registry; no key in any page or
//! response; a read-only token can only read.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use rung_gateway::{Access, Config, Instance, Role, Settings, doors, role};
use rung_testkit::TempDir;

const OWNER_KEY: &str = "KEY-canary-9f31c0de-owner-key-of-alpha";
const READ_TOKEN: &str = "view-token-canary-77aa";

#[derive(Debug, Clone)]
struct Seen {
    method: String,
    target: String,
    authorization: Option<String>,
    cookie: Option<String>,
    last_event_id: Option<String>,
    body: String,
}

type Log = Arc<Mutex<Vec<Seen>>>;

/// A stand-in host: records each request it gets and answers the doors the
/// tests use.
async fn stub_host() -> (SocketAddr, Log) {
    let log: Log = Arc::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let l2 = log.clone();
    tokio::spawn(async move {
        loop {
            let (s, _) = listener.accept().await.unwrap();
            let log = l2.clone();
            tokio::spawn(async move {
                let svc = hyper::service::service_fn(move |req: Request<Incoming>| {
                    let log = log.clone();
                    async move { Ok::<_, Infallible>(host_answer(req, log).await) }
                });
                let _ = Builder::new(TokioExecutor::new())
                    .serve_connection(TokioIo::new(s), svc)
                    .await;
            });
        }
    });
    (addr, log)
}

type Out = Response<http_body_util::combinators::BoxBody<Bytes, Infallible>>;

async fn host_answer(req: Request<Incoming>, log: Log) -> Out {
    let h = |n: &str| {
        req.headers()
            .get(n)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    };
    let (authorization, cookie, last_event_id) =
        (h("authorization"), h("cookie"), h("last-event-id"));
    let method = req.method().to_string();
    let target = req.uri().path_and_query().unwrap().to_string();
    let path = req.uri().path().to_string();
    let body = req.into_body().collect().await.unwrap().to_bytes();
    log.lock().unwrap().push(Seen {
        method,
        target,
        authorization,
        cookie,
        last_event_id,
        body: String::from_utf8_lossy(&body).into(),
    });
    match path.as_str() {
        "/v1/summary" => Response::builder()
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from_static(br#"{"state":"Working","line":"on it"}"#)).boxed())
            .unwrap(),
        "/v1/events" => {
            let (tx, rx) = tokio::sync::mpsc::channel::<Result<Frame<Bytes>, Infallible>>(4);
            tokio::spawn(async move {
                let _ = tx
                    .send(Ok(Frame::data(Bytes::from_static(
                        b"id: 1\ndata: first\n\n",
                    ))))
                    .await;
                tokio::time::sleep(Duration::from_millis(1500)).await;
                let _ = tx
                    .send(Ok(Frame::data(Bytes::from_static(
                        b"id: 2\ndata: second\n\n",
                    ))))
                    .await;
            });
            Response::builder()
                .header("content-type", "text/event-stream")
                .body(StreamBody::new(tokio_stream_from(rx)).boxed())
                .unwrap()
        }
        _ => Response::builder()
            .status(200)
            .body(Full::new(Bytes::from_static(b"ok")).boxed())
            .unwrap(),
    }
}

fn tokio_stream_from(
    rx: tokio::sync::mpsc::Receiver<Result<Frame<Bytes>, Infallible>>,
) -> impl futures::Stream<Item = Result<Frame<Bytes>, Infallible>> {
    futures::stream::unfold(rx, |mut rx| async { rx.recv().await.map(|x| (x, rx)) })
}

fn instance(id: &str, addr: SocketAddr) -> Instance {
    Instance {
        id: id.into(),
        name: format!("Instance {id}"),
        url: format!("http://{addr}"),
        key: OWNER_KEY.into(),
    }
}

fn settings(app_dir: Option<std::path::PathBuf>, instances: Vec<Instance>) -> Settings {
    Settings {
        listen: "127.0.0.1:0".parse().unwrap(),
        app_dir,
        allowed_hosts: vec![],
        read_only_tokens: vec![READ_TOKEN.into()],
        instances,
    }
}

async fn gateway(
    app_dir: Option<std::path::PathBuf>,
    instances: Vec<Instance>,
) -> rung_gateway::Running {
    rung_gateway::start(settings(app_dir, instances))
        .await
        .unwrap()
}

fn url(g: &rung_gateway::Running, path: &str) -> String {
    format!("http://{}{path}", g.addr)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

/// A port nothing listens on.
fn dead_addr() -> SocketAddr {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap()
}

#[tokio::test]
async fn instances_door_lists_the_registry_with_each_summary() {
    let (host, _) = stub_host().await;
    let g = gateway(
        None,
        vec![instance("alpha", host), instance("beta", dead_addr())],
    )
    .await;
    let r = client()
        .get(url(&g, "/api/instances"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v: serde_json::Value = r.json().await.unwrap();
    let list = v["instances"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["id"], "alpha");
    assert_eq!(list[0]["name"], "Instance alpha");
    assert_eq!(list[0]["reachable"], true);
    assert_eq!(list[0]["summary"]["state"], "Working");
    // an unreachable instance is listed, not missing
    assert_eq!(list[1]["id"], "beta");
    assert_eq!(list[1]["reachable"], false);
    assert!(list[1]["summary"].is_null());
    assert!(list[1]["error"].is_string());
    let text = v.to_string();
    assert!(!text.contains(OWNER_KEY), "the key must not appear");
}

#[tokio::test]
async fn proxy_adds_the_instance_key_and_drops_the_clients_credentials() {
    let (host, log) = stub_host().await;
    let g = gateway(None, vec![instance("alpha", host)]).await;
    let r = client()
        .get(url(&g, "/api/i/alpha/v1/record?offset=5&limit=2"))
        .header("authorization", format!("Bearer {READ_TOKEN}"))
        .header("cookie", "session=abc")
        .header("last-event-id", "41")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.text().await.unwrap(), "ok");
    let seen = log.lock().unwrap().last().cloned().unwrap();
    assert_eq!(seen.method, "GET");
    assert_eq!(seen.target, "/v1/record?offset=5&limit=2");
    assert_eq!(
        seen.authorization.as_deref(),
        Some(&*format!("Bearer {OWNER_KEY}"))
    );
    assert_eq!(seen.cookie, None);
    assert_eq!(
        seen.last_event_id.as_deref(),
        Some("41"),
        "resume header passes"
    );
}

#[tokio::test]
async fn proxy_carries_writes_with_their_body() {
    let (host, log) = stub_host().await;
    let g = gateway(None, vec![instance("alpha", host)]).await;
    let r = client()
        .post(url(&g, "/api/i/alpha/v1/queue"))
        .body(r#"{"text":"hello"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let seen = log.lock().unwrap().last().cloned().unwrap();
    assert_eq!(
        (seen.method.as_str(), seen.target.as_str()),
        ("POST", "/v1/queue")
    );
    assert_eq!(seen.body, r#"{"text":"hello"}"#);
}

#[tokio::test]
async fn proxy_streams_without_buffering() {
    let (host, _) = stub_host().await;
    let g = gateway(None, vec![instance("alpha", host)]).await;
    let t0 = Instant::now();
    let mut r = client()
        .get(url(&g, "/api/i/alpha/v1/events?after=0"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.headers()["content-type"], "text/event-stream");
    let first = r.chunk().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&first).contains("data: first"));
    assert!(
        t0.elapsed() < Duration::from_millis(1000),
        "first event waited for the stream's end"
    );
    let second = r.chunk().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&second).contains("data: second"));
}

#[tokio::test]
async fn only_v1_of_a_known_instance_passes() {
    let (host, log) = stub_host().await;
    let g = gateway(None, vec![instance("alpha", host)]).await;
    for path in [
        "/api/i/nope/v1/summary",
        "/api/i/alpha/acp",
        "/api/i/alpha/",
        "/api/i/alpha",
        "/api/i/alpha/v1/../acp",
        "/api/i/alpha/v1/%2e%2e/acp",
        "/api/i/alpha/v1/%2E%2E%2Facp",
        "/api/nothing",
    ] {
        let r = client().get(url(&g, path)).send().await.unwrap();
        assert_eq!(r.status(), 404, "{path}");
        let v: serde_json::Value = r.json().await.unwrap();
        assert!(v["error"].is_string(), "{path}: a JSON error");
    }
    assert!(log.lock().unwrap().is_empty(), "nothing reached the host");
}

#[tokio::test]
async fn an_unreachable_instance_answers_502_without_its_key() {
    let g = gateway(None, vec![instance("dead", dead_addr())]).await;
    let r = client()
        .get(url(&g, "/api/i/dead/v1/summary"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 502);
    let text = r.text().await.unwrap();
    assert!(text.contains("instance_unreachable"));
    assert!(!text.contains(OWNER_KEY));
}

#[tokio::test]
async fn read_only_token_reads_but_cannot_write() {
    let (host, log) = stub_host().await;
    let g = gateway(None, vec![instance("alpha", host)]).await;
    let bearer = format!("Bearer {READ_TOKEN}");
    let c = client();
    // reads pass
    let r = c
        .get(url(&g, "/api/i/alpha/v1/turns"))
        .header("authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    // writes, by any door or an unnamed one, are refused before the host
    let before = log.lock().unwrap().len();
    for (m, p) in [
        ("POST", "/api/i/alpha/v1/queue"),
        ("DELETE", "/api/i/alpha/v1/queue/7"),
        ("POST", "/api/i/alpha/v1/queue/7/move"),
        ("PUT", "/api/i/alpha/v1/config"),
        ("POST", "/api/i/alpha/v1/config/validate"),
        ("POST", "/api/i/alpha/v1/stop"),
        ("POST", "/api/i/alpha/v1/release"),
        ("POST", "/api/i/alpha/v1/never-heard-of-it"),
        ("DELETE", "/api/i/alpha/v1/turns"),
    ] {
        let r = c
            .request(m.parse().unwrap(), url(&g, p))
            .header("authorization", &bearer)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403, "{m} {p}");
        let v: serde_json::Value = r.json().await.unwrap();
        assert_eq!(v["error"], "read_only");
    }
    assert_eq!(
        log.lock().unwrap().len(),
        before,
        "no write reached the host"
    );
}

#[tokio::test]
async fn the_token_may_ride_the_query_and_is_not_forwarded() {
    let (host, log) = stub_host().await;
    let g = gateway(None, vec![instance("alpha", host)]).await;
    let c = client();
    let r = c
        .get(url(
            &g,
            &format!("/api/i/alpha/v1/events?after=3&token={READ_TOKEN}"),
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let seen = log.lock().unwrap().last().cloned().unwrap();
    assert_eq!(seen.target, "/v1/events?after=3");
    let r = c
        .post(url(
            &g,
            &format!("/api/i/alpha/v1/queue?token={READ_TOKEN}"),
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn no_token_is_the_owner_and_an_unknown_token_is_refused() {
    let (host, log) = stub_host().await;
    let g = gateway(None, vec![instance("alpha", host)]).await;
    let c = client();
    let r = c
        .post(url(&g, "/api/i/alpha/v1/queue"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        200,
        "the tailnet is the boundary: no token, full role"
    );
    let n = log.lock().unwrap().len();
    let r = c
        .get(url(&g, "/api/i/alpha/v1/summary"))
        .header("authorization", "Bearer not-a-token")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let r = c
        .get(url(&g, "/api/instances?token=also-not"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    assert_eq!(log.lock().unwrap().len(), n);
}

#[tokio::test]
async fn read_role_sees_the_instances_door_and_cannot_post_to_it() {
    let g = gateway(None, vec![]).await;
    let c = client();
    let r = c
        .get(url(&g, "/api/instances"))
        .header("authorization", format!("Bearer {READ_TOKEN}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let r = c
        .post(url(&g, "/api/instances"))
        .header("authorization", format!("Bearer {READ_TOKEN}"))
        .send()
        .await
        .unwrap();
    assert!(r.status() == 403 || r.status() == 405);
}

#[tokio::test]
async fn serves_the_app_with_a_single_page_fallback() {
    let dir = TempDir::new("gw-app");
    std::fs::write(dir.path().join("index.html"), "<html>app</html>").unwrap();
    std::fs::create_dir_all(dir.path().join("assets")).unwrap();
    std::fs::write(dir.path().join("assets/app.js"), "console.log(1)").unwrap();
    std::fs::write(dir.path().join("secret.txt"), "outside").unwrap();
    let app = dir.path().join("assets");
    // serve `assets` as the app dir to give traversal something to reach
    std::fs::write(app.join("index.html"), "<html>app</html>").unwrap();
    let g = gateway(Some(app.clone()), vec![]).await;
    let c = client();
    let r = c.get(url(&g, "/")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert!(
        r.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(r.text().await.unwrap(), "<html>app</html>");
    let r = c.get(url(&g, "/app.js")).send().await.unwrap();
    assert!(
        r.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("javascript")
    );
    assert_eq!(r.text().await.unwrap(), "console.log(1)");
    // a client-side route falls back to the page
    let r = c.get(url(&g, "/i/alpha/turns")).send().await.unwrap();
    assert_eq!(r.text().await.unwrap(), "<html>app</html>");
    // a missing file is a 404, not the page
    assert_eq!(
        c.get(url(&g, "/missing.js")).send().await.unwrap().status(),
        404
    );
    // traversal never leaves the app dir
    for p in [
        "/../secret.txt",
        "/%2e%2e/secret.txt",
        "/%2e%2e%2fsecret.txt",
        "/..%5csecret.txt",
    ] {
        let r = c.get(url(&g, p)).send().await.unwrap();
        assert!(!r.text().await.unwrap().contains("outside"), "{p}");
    }
    // the API namespace is never the page
    let r = c.get(url(&g, "/api/whatever")).send().await.unwrap();
    assert_eq!(r.status(), 404);
    assert!(r.text().await.unwrap().contains("error"));
}

#[tokio::test]
async fn without_an_app_dir_the_root_says_so() {
    let g = gateway(None, vec![]).await;
    let r = client().get(url(&g, "/")).send().await.unwrap();
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn no_key_in_any_page_or_response() {
    let dir = TempDir::new("gw-canary");
    std::fs::write(dir.path().join("index.html"), "<html>app</html>").unwrap();
    let (host, _) = stub_host().await;
    let s = settings(
        Some(dir.path().to_path_buf()),
        vec![instance("alpha", host), instance("dead", dead_addr())],
    );
    assert!(
        !format!("{s:?}").contains(OWNER_KEY),
        "Debug must not print a key"
    );
    assert!(
        !format!("{s:?}").contains(READ_TOKEN),
        "Debug must not print a token"
    );
    let g = rung_gateway::start(s).await.unwrap();
    let c = client();
    let bearer = format!("Bearer {READ_TOKEN}");
    for (m, p) in [
        ("GET", "/"),
        ("GET", "/api/instances"),
        ("GET", "/api/health"),
        ("GET", "/api/i/alpha/v1/summary"),
        ("GET", "/api/i/dead/v1/summary"),
        ("GET", "/api/i/nope/v1/summary"),
        ("POST", "/api/i/alpha/v1/queue"),
        ("GET", "/api/nothing"),
    ] {
        for auth in [None, Some(bearer.as_str()), Some("Bearer junk")] {
            let mut rq = c.request(m.parse().unwrap(), url(&g, p));
            if let Some(a) = auth {
                rq = rq.header("authorization", a);
            }
            let r = rq.send().await.unwrap();
            let heads = format!("{:?}", r.headers());
            let body = r.text().await.unwrap();
            assert!(
                !body.contains(OWNER_KEY) && !heads.contains(OWNER_KEY),
                "{m} {p}"
            );
        }
    }
}

#[test]
fn the_door_table_classifies_every_door_of_the_note() {
    use Access::*;
    for (m, p, want) in [
        ("GET", "/v1/summary", Read),
        ("GET", "/v1/status", Read),
        ("GET", "/v1/report", Read),
        ("GET", "/v1/record", Read),
        ("GET", "/v1/turns", Read),
        ("GET", "/v1/turns/12", Read),
        ("GET", "/v1/decisions", Read),
        ("GET", "/v1/pack", Read),
        ("GET", "/v1/spend", Read),
        ("GET", "/v1/events", Read),
        ("GET", "/v1/queue", Read),
        ("GET", "/v1/config", Read),
        ("GET", "/v1/console", Read),
        ("GET", "/v1/console/stream", Read),
        ("POST", "/v1/queue", Write),
        ("DELETE", "/v1/queue/9", Write),
        ("POST", "/v1/queue/9/move", Write),
        ("POST", "/v1/config/validate", Write),
        ("PUT", "/v1/config", Write),
        ("POST", "/v1/stop", Write),
        ("POST", "/v1/release", Write),
        // not in the table: by method
        ("GET", "/v1/future-door", Read),
        ("HEAD", "/v1/future-door", Read),
        ("POST", "/v1/future-door", Write),
        ("PATCH", "/v1/turns", Write),
        ("DELETE", "/v1/turns", Write),
    ] {
        assert_eq!(doors::classify(m, p), want, "{m} {p}");
    }
}

#[test]
fn every_door_is_a_v1_door_with_one_row() {
    assert!(doors::DOORS.iter().all(|d| d.path.starts_with("/v1/")));
    for (i, d) in doors::DOORS.iter().enumerate() {
        assert!(
            doors::DOORS[i + 1..]
                .iter()
                .all(|e| (e.method, e.path) != (d.method, d.path)),
            "{} {} is listed twice",
            d.method,
            d.path
        );
    }
}

#[test]
fn roles_resolve_from_header_or_query() {
    let ro = vec![READ_TOKEN.to_string()];
    use role::Who::*;
    assert_eq!(role::resolve(None, None, &ro), Known(Role::Owner));
    assert_eq!(
        role::resolve(Some(READ_TOKEN), None, &ro),
        Known(Role::ReadOnly)
    );
    assert_eq!(
        role::resolve(None, Some(READ_TOKEN), &ro),
        Known(Role::ReadOnly)
    );
    assert_eq!(role::resolve(Some("x"), None, &ro), Unknown);
    assert_eq!(
        role::resolve(Some(READ_TOKEN), Some("x"), &ro),
        Unknown,
        "a bad token anywhere refuses"
    );
    assert_eq!(role::resolve(Some(""), None, &ro), Unknown);
    assert_eq!(role::resolve(None, None, &[]), Known(Role::Owner));
    assert_eq!(
        role::resolve(Some("x"), None, &[]),
        Unknown,
        "no read token configured: any token is unknown"
    );
}

fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let m: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |k| m.get(k).cloned()
}

#[test]
fn config_names_variables_and_resolves_them() {
    let dir = TempDir::new("gw-cfg");
    let file = dir.path().join("gateway.yaml");
    std::fs::write(
        &file,
        "listen: 127.0.0.1:9000\napp_dir: dist\nread_only_token_envs: [VIEW]\ninstances:\n  - id: alpha\n    name: Alpha\n    url: http://127.0.0.1:7001/\n    key_env: ALPHA_KEY\n  - id: b-2\n    url: http://127.0.0.1:7002\n    key_env: B_KEY\n",
    )
    .unwrap();
    let cfg = Config::load(&file).unwrap();
    assert_eq!(
        cfg.app_dir.as_deref(),
        Some(dir.path().join("dist").as_path()),
        "anchored beside the file"
    );
    let s = cfg
        .resolve(env_of(&[
            ("VIEW", "t1"),
            ("ALPHA_KEY", "ka"),
            ("B_KEY", "kb"),
        ]))
        .unwrap();
    assert_eq!(s.listen, "127.0.0.1:9000".parse().unwrap());
    assert_eq!(s.read_only_tokens, vec!["t1".to_string()]);
    assert_eq!(s.instances.len(), 2);
    assert_eq!(s.instances[0].name, "Alpha");
    assert_eq!(
        s.instances[0].url, "http://127.0.0.1:7001",
        "no trailing slash"
    );
    assert_eq!(s.instances[0].key, "ka");
    assert_eq!(s.instances[1].name, "b-2", "the name defaults to the id");
}

#[test]
fn config_refuses_what_it_cannot_serve_naming_the_variable_never_a_value() {
    let base = |extra: &str| {
        format!(
            "instances:\n  - id: alpha\n    url: http://127.0.0.1:1\n    key_env: ALPHA_KEY\n{extra}"
        )
    };
    let parse = |y: &str| -> Config { serde_yaml::from_str(y).unwrap() };
    // a key that cannot ride a header (CR, LF, NUL, non-ASCII) is refused
    for bad in ["a\r\nb", "a\nb", "a\0b", "k\u{e9}y"] {
        let e = parse(&base(""))
            .resolve(env_of(&[("ALPHA_KEY", bad)]))
            .unwrap_err();
        assert!(e.contains("ALPHA_KEY") && !e.contains(bad), "{e:?}");
    }
    // a missing key variable
    let e = parse(&base("")).resolve(env_of(&[])).unwrap_err();
    assert!(e.contains("ALPHA_KEY"), "{e}");
    // an empty one
    let e = parse(&base(""))
        .resolve(env_of(&[("ALPHA_KEY", "")]))
        .unwrap_err();
    assert!(e.contains("ALPHA_KEY"), "{e}");
    // a missing token variable
    let e = parse(&base("read_only_token_envs: [VIEW]\n"))
        .resolve(env_of(&[("ALPHA_KEY", "secretvalue")]))
        .unwrap_err();
    assert!(e.contains("VIEW") && !e.contains("secretvalue"), "{e}");
    // a duplicate id, a bad id, a non-http url, a bad listen
    let two = "instances:\n  - id: a\n    url: http://h:1\n    key_env: K\n  - id: a\n    url: http://h:2\n    key_env: K\n";
    assert!(
        parse(two)
            .resolve(env_of(&[("K", "v")]))
            .unwrap_err()
            .contains("duplicate")
    );
    let bad = "instances:\n  - id: 'a/b'\n    url: http://h:1\n    key_env: K\n";
    assert!(parse(bad).resolve(env_of(&[("K", "v")])).is_err());
    let tls = "instances:\n  - id: a\n    url: https://h:1\n    key_env: K\n";
    assert!(
        parse(tls)
            .resolve(env_of(&[("K", "v")]))
            .unwrap_err()
            .contains("http://")
    );
    assert!(parse("listen: nonsense\n").resolve(env_of(&[])).is_err());
    // unknown fields are a mistake, not ignored
    assert!(serde_yaml::from_str::<Config>("instance: []\n").is_err());
}

#[tokio::test]
async fn a_key_that_cannot_ride_a_header_answers_502_not_a_panic() {
    // a registry may hand over any key; the gateway must answer, not drop the connection
    for bad in ["bad\r\nkey", "bad\0key", "k\u{e9}y"] {
        let (host, log) = stub_host().await;
        let mut inst = instance("alpha", host);
        inst.key = bad.into();
        let g = gateway(None, vec![inst]).await;
        let r = client()
            .get(url(&g, "/api/i/alpha/v1/summary"))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 502, "{bad:?}");
        let text = r.text().await.unwrap();
        assert!(text.contains("instance_unreachable") && !text.contains(bad));
        assert!(log.lock().unwrap().is_empty(), "nothing reached the host");
        // the overview lists it as unreachable
        let v: serde_json::Value = client()
            .get(url(&g, "/api/instances"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(v["instances"][0]["reachable"], false);
        assert!(!v.to_string().contains(bad));
        // and the gateway keeps serving
        assert_eq!(
            client()
                .get(url(&g, "/api/health"))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
}

// ---- the owner role's write guard: Origin and Host ------------------------

async fn writes_gateway(allowed: &[&str]) -> (rung_gateway::Running, Log) {
    let (host, log) = stub_host().await;
    let mut s = settings(None, vec![instance("alpha", host)]);
    s.allowed_hosts = allowed.iter().map(|h| h.to_string()).collect();
    (rung_gateway::start(s).await.unwrap(), log)
}

#[tokio::test]
async fn a_write_from_another_origin_is_refused() {
    let (g, log) = writes_gateway(&[]).await;
    let me = format!("http://{}", g.addr);
    for origin in [
        "http://evil.example",
        "https://evil.example:8443",
        "null",
        "http://127.0.0.1.evil.example",
        "garbage",
    ] {
        let r = client()
            .post(url(&g, "/api/i/alpha/v1/queue"))
            .header("origin", origin)
            .body("x")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403, "{origin}");
        let v: serde_json::Value = r.json().await.unwrap();
        assert_eq!(v["error"], "bad_origin", "{origin}");
    }
    // every write method is guarded, not just POST
    for m in ["PUT", "DELETE", "PATCH"] {
        let r = client()
            .request(m.parse().unwrap(), url(&g, "/api/i/alpha/v1/queue/1"))
            .header("origin", "http://evil.example")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403, "{m}");
    }
    assert!(log.lock().unwrap().is_empty(), "no write reached the host");
    // the page's own origin, and a client that sends no Origin (a script), pass
    let r = client()
        .post(url(&g, "/api/i/alpha/v1/queue"))
        .header("origin", &me)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let r = client()
        .post(url(&g, "/api/i/alpha/v1/queue"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn a_write_addressed_to_a_foreign_host_is_refused() {
    let (g, log) = writes_gateway(&[]).await;
    // DNS rebinding: the page's origin and Host agree on a name that is not ours
    for (host, origin) in [
        ("evil.example", None),
        ("evil.example:8787", Some("http://evil.example:8787")),
        ("10.9.9.9", None),
    ] {
        let mut rq = client()
            .post(url(&g, "/api/i/alpha/v1/queue"))
            .header("host", host);
        if let Some(o) = origin {
            rq = rq.header("origin", o);
        }
        let r = rq.send().await.unwrap();
        assert_eq!(r.status(), 403, "{host}");
        let v: serde_json::Value = r.json().await.unwrap();
        assert_eq!(v["error"], "bad_host", "{host}");
    }
    assert!(log.lock().unwrap().is_empty());
    // loopback names pass, with or without a port, in any case
    for host in ["localhost", "LOCALHOST:9", "127.0.0.1", "[::1]:8787"] {
        let r = client()
            .post(url(&g, "/api/i/alpha/v1/queue"))
            .header("host", host)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200, "{host}");
    }
}

#[tokio::test]
async fn a_configured_host_may_be_written_to_from_its_own_origin_only() {
    let (g, log) = writes_gateway(&["gw.tailnet.example"]).await;
    let post = |host: &str, origin: Option<&str>| {
        let mut rq = client()
            .post(url(&g, "/api/i/alpha/v1/queue"))
            .header("host", host);
        if let Some(o) = origin {
            rq = rq.header("origin", o);
        }
        rq.send()
    };
    assert_eq!(
        post("gw.tailnet.example", Some("https://gw.tailnet.example"))
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        post("GW.tailnet.example:443", None).await.unwrap().status(),
        200
    );
    // a page on another origin cannot write through an allowed Host
    assert_eq!(
        post("gw.tailnet.example", Some("https://evil.example"))
            .await
            .unwrap()
            .status(),
        403
    );
    // and an allowed origin cannot ride a different Host
    assert_eq!(
        post("127.0.0.1", Some("https://gw.tailnet.example"))
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(log.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn reads_and_the_read_only_role_are_not_subject_to_the_write_guard() {
    let (g, _) = writes_gateway(&[]).await;
    // a read from another origin or host still answers (the guard is for writes)
    let r = client()
        .get(url(&g, "/api/health"))
        .header("origin", "http://evil.example")
        .header("host", "evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    // the read-only role keeps its own refusal, whatever the origin
    let r = client()
        .post(url(&g, "/api/i/alpha/v1/queue"))
        .header("authorization", format!("Bearer {READ_TOKEN}"))
        .header("origin", "http://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
    let v: serde_json::Value = r.json().await.unwrap();
    assert_eq!(v["error"], "read_only");
}

#[test]
fn config_reads_allowed_hosts_as_names_only() {
    let parse = |y: &str| -> Config { serde_yaml::from_str(y).unwrap() };
    let s = parse("allowed_hosts: [GW.Tailnet.Example]\n")
        .resolve(env_of(&[]))
        .unwrap();
    assert_eq!(s.allowed_hosts, vec!["gw.tailnet.example".to_string()]);
    for bad in [
        "https://gw.example",
        "gw.example:8787",
        "gw.example/x",
        "",
        "u@gw.example",
    ] {
        let y = format!("allowed_hosts: ['{bad}']\n");
        let e = parse(&y).resolve(env_of(&[])).unwrap_err();
        assert!(e.contains("allowed_hosts"), "{bad}: {e}");
    }
}
