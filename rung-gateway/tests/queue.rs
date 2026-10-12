//! S1: the gateway's write path to a native instance, and the queue doors.
//!
//! `GET /api/i/{id}/v1/queue` is computed from the instance's own record, so it
//! reads the same for a stopped instance. `POST` is forwarded to the running
//! host's own listener (the address of its registry entry) with an owner key the
//! gateway holds by name: the variable the host's config file names for its
//! `owner` token. A "running" host is one whose state lock the test holds, and
//! its listener is a small fake that speaks the host's door: it writes the item
//! to the record before it answers. The key is a planted sentinel that no gateway
//! output may carry; no assertion message prints a sentinel.

use std::convert::Infallible;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use rung_gateway::{Settings, StaticRegistry, Tuning};
use rung_host::statelock::StateLock;
use rung_testkit::TempDir;
use serde_json::{Value, json};
use tokio::net::TcpListener;

const READ_TOKEN: &str = "view-token-canary-s1-4412";
/// The owner key the host's config names, planted: no output may carry it.
const OWNER_KEY: &str = "plainownerkeys1-0123456789-zzz";
/// A variable the host's own config names for its owner token: the gateway must
/// never read it, let alone send it anywhere.
const PLANTED: &str = "plantedhostconfigvaluexx-0123456789";
const SHAPED: &str = "sk-SENTINELs1gwshaped0123456789abcdef";
const T0: i64 = 1_791_208_186_000;

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

// ─── the world ───────────────────────────────────────────────────────────────

struct Fleet {
    root: TempDir,
    locks: Vec<StateLock>,
    /// The gateway's own config: the instances it holds a write key for.
    twins: Vec<rung_gateway::Instance>,
}

impl Fleet {
    fn new(tag: &str) -> Self {
        let root = TempDir::new(tag);
        std::fs::create_dir_all(root.path().join("registry")).unwrap();
        Fleet {
            root,
            locks: Vec::new(),
            twins: Vec::new(),
        }
    }

    fn registry(&self) -> PathBuf {
        self.root.path().join("registry")
    }

    fn state(&self, id: &str) -> PathBuf {
        self.root.path().join(id).join("state")
    }

    /// The host's config file, naming the variable that holds its owner token.
    fn config(&self, id: &str, token_env: Option<&str>) -> PathBuf {
        let p = self.root.path().join(format!("{id}.yaml"));
        let tokens = match token_env {
            Some(e) => format!("  tokens:\n    owner: {e}\n"),
            None => String::new(),
        };
        std::fs::write(
            &p,
            format!(
                "name: x\nstate: /x\nengine: {{kind: agent}}\nacp:\n  http: 127.0.0.1:0\n{tokens}"
            ),
        )
        .unwrap();
        p
    }

    fn add(&self, id: &str, config: Option<&Path>, address: Option<&str>) {
        let state = self.state(id);
        std::fs::create_dir_all(state.join("record")).unwrap();
        let entry = json!({
            "schema": 1, "id": id, "name": id.to_uppercase(),
            "state_dir": state, "config": config,
            "workspace": self.root.path().join(id).join("workspace"),
            "address": address, "pid": 4242, "started": T0, "version": "0.2.3",
        });
        std::fs::write(
            self.registry().join(format!("{id}.json")),
            serde_json::to_string_pretty(&entry).unwrap(),
        )
        .unwrap();
    }

    fn hold(&mut self, id: &str) {
        self.locks
            .push(StateLock::acquire(&self.state(id)).expect("lock"));
    }

    /// Lines with consecutive numbers from 1.
    fn record(&self, id: &str, lines: Vec<(&str, Value)>) {
        let mut f =
            std::fs::File::create(self.state(id).join("record/seg-00000001.ndjson")).unwrap();
        for (i, (kind, body)) in lines.into_iter().enumerate() {
            let mut m = body.as_object().cloned().unwrap_or_default();
            m.insert("seq".into(), (i as u64 + 1).into());
            m.insert("at".into(), (T0 + i as i64 * 1000).into());
            m.insert("kind".into(), kind.into());
            writeln!(f, "{}", Value::Object(m)).unwrap();
        }
    }

    fn settings(&self) -> Settings {
        Settings {
            listen: "127.0.0.1:0".parse().unwrap(),
            app_dir: None,
            allowed_hosts: vec![],
            read_only_tokens: vec![READ_TOKEN.into()],
            registry_dir: Some(self.registry()),
            instances: self.twins.clone(),
        }
    }

    /// The gateway's config names `id`, with the key it holds for it.
    fn twin(&mut self, id: &str, url: &str, key: &str) {
        self.twins.push(rung_gateway::Instance {
            id: id.into(),
            name: id.into(),
            url: format!("http://{url}"),
            key: key.into(),
        });
    }

    async fn serve(&self) -> Gw {
        self.serve_tuned(Tuning::default()).await
    }

    async fn serve_tuned(&self, tuning: Tuning) -> Gw {
        let running = rung_gateway::start_tuned(
            self.settings(),
            Arc::new(StaticRegistry(self.twins.clone())),
            tuning,
        )
        .await
        .unwrap();
        Gw {
            base: format!("http://{}", running.addr),
            _running: running,
        }
    }
}

fn accepted_line(
    id: &str,
    role: &str,
    channel: &str,
    at: i64,
    text: &str,
) -> (&'static str, Value) {
    (
        "stimulus.accepted",
        json!({"item": {"id": id, "kind": "peer", "role": role, "channel": channel,
                        "at": at, "firm": false, "text": text}}),
    )
}

struct Gw {
    base: String,
    _running: rung_gateway::Running,
}

struct Answer {
    status: u16,
    body: String,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("not JSON ({e}): {}", self.body))
    }
}

async fn send(rb: reqwest::RequestBuilder) -> Answer {
    let r = rb.send().await.unwrap();
    Answer {
        status: r.status().as_u16(),
        body: r.text().await.unwrap(),
    }
}

impl Gw {
    fn url(&self, id: &str, door: &str) -> String {
        format!("{}/api/i/{id}/v1/{door}", self.base)
    }

    async fn get(&self, id: &str, door: &str) -> Answer {
        send(client().get(self.url(id, door))).await
    }

    async fn post(&self, id: &str, body: &Value) -> Answer {
        send(
            client()
                .post(self.url(id, "queue"))
                .header("content-type", "application/json")
                .body(body.to_string()),
        )
        .await
    }
}

// ─── a fake running host ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Takes the item, writes it to the record, answers.
    Normal,
    /// Refuses, and puts the bearer it was given in its answer (a faulty host).
    EchoKey,
    /// Reads the request and never answers.
    Silent,
}

struct FakeHost {
    addr: String,
    hits: Arc<AtomicUsize>,
    seen: Arc<std::sync::Mutex<Vec<(String, String)>>>,
    /// The bearer of the last request, without `Bearer `.
    last_bearer: Arc<std::sync::Mutex<Option<String>>>,
    /// The body of each request, in the order they came.
    bodies: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
}

fn record_lines(state: &Path) -> Vec<Value> {
    std::fs::read_to_string(state.join("record/seg-00000001.ndjson"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

async fn fake_host(state: PathBuf, mode: Mode) -> FakeHost {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let hits = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let last_bearer = Arc::new(std::sync::Mutex::new(None));
    let bodies = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (h2, s2, b2, bo2) = (
        hits.clone(),
        seen.clone(),
        last_bearer.clone(),
        bodies.clone(),
    );
    tokio::spawn(async move {
        loop {
            let (sock, _) = l.accept().await.unwrap();
            let (state, hits, seen, lb, bo) = (
                state.clone(),
                h2.clone(),
                s2.clone(),
                b2.clone(),
                bo2.clone(),
            );
            tokio::spawn(async move {
                let svc = hyper::service::service_fn(move |req: Request<hyper::body::Incoming>| {
                    let (state, hits, seen, lb, bo) = (
                        state.clone(),
                        hits.clone(),
                        seen.clone(),
                        lb.clone(),
                        bo.clone(),
                    );
                    async move {
                        hits.fetch_add(1, Ordering::SeqCst);
                        let bearer = req
                            .headers()
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("")
                            .to_string();
                        *lb.lock().unwrap() = bearer.strip_prefix("Bearer ").map(str::to_string);
                        let all_headers = format!("{:?}", req.headers());
                        seen.lock().unwrap().push((
                            format!("{} {}", req.method(), req.uri().path()),
                            all_headers,
                        ));
                        let reply = |status: u16, v: Value| {
                            Ok::<_, Infallible>(
                                Response::builder()
                                    .status(status)
                                    .header("content-type", "application/json")
                                    .body(Full::new(Bytes::from(v.to_string())))
                                    .unwrap(),
                            )
                        };
                        let path = req.uri().path().to_string();
                        let body = req.into_body().collect().await.unwrap().to_bytes();
                        bo.lock().unwrap().push(body.to_vec());
                        if mode == Mode::Silent {
                            std::future::pending::<()>().await;
                        }
                        if mode == Mode::EchoKey {
                            return reply(
                                500,
                                json!({"error": "boom", "message": format!("got {bearer}")}),
                            );
                        }
                        if bearer != format!("Bearer {OWNER_KEY}") {
                            return reply(401, json!({"error": "unauthorized", "message": "no"}));
                        }
                        // a move or a cancel: the fake answers as a host that did it
                        if path != "/v1/queue" {
                            return reply(200, json!({"done": path}));
                        }
                        let v: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                        let lines = record_lines(&state);
                        let n = lines.len();
                        let id = v["key"]
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("gen-{n}"));
                        let item = json!({"id": id, "kind": "peer", "role": "owner",
                                          "channel": "owner", "at": T0 + n as i64,
                                          "firm": false, "text": v["text"]});
                        let known = lines
                            .iter()
                            .any(|l| l["kind"] == "stimulus.accepted" && l["item"]["id"] == id);
                        if !known {
                            let line = json!({"seq": n + 1, "at": T0 + n as i64,
                                              "kind": "stimulus.accepted", "item": item});
                            let mut f = std::fs::OpenOptions::new()
                                .append(true)
                                .create(true)
                                .open(state.join("record/seg-00000001.ndjson"))
                                .unwrap();
                            writeln!(f, "{line}").unwrap();
                            f.sync_all().unwrap();
                        }
                        reply(
                            if known { 200 } else { 201 },
                            json!({"id": id, "repeat": known, "item": item}),
                        )
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(sock), svc)
                    .await;
            });
        }
    });
    FakeHost {
        addr,
        hits,
        seen,
        last_bearer,
        bodies,
    }
}

/// A running instance `id`: its owner token named by a variable this process
/// holds, its lock held, its listener a fake.
async fn running(f: &mut Fleet, id: &str, mode: Mode) -> FakeHost {
    // the host's own config names a variable this process holds: it is NOT the
    // key the gateway sends (that is the gateway's own config's)
    let env = format!("S1GW_HOSTCFG_{}", id.to_uppercase());
    unsafe { std::env::set_var(&env, PLANTED) };
    let cfg = f.config(id, Some(&env));
    let fake = fake_host(f.state(id), mode).await;
    f.add(id, Some(&cfg), Some(&fake.addr));
    f.twin(id, &fake.addr, OWNER_KEY);
    f.hold(id);
    fake
}

fn whole_record(f: &Fleet, id: &str) -> String {
    let dir = f.state(id).join("record");
    let mut out = String::new();
    for e in std::fs::read_dir(dir).unwrap() {
        out.push_str(&std::fs::read_to_string(e.unwrap().path()).unwrap());
    }
    out
}

// ─── add through the gateway ─────────────────────────────────────────────────

#[tokio::test]
async fn an_added_item_is_in_the_record_by_the_time_the_gateway_answers() {
    let mut f = Fleet::new("s1gw-add");
    let fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let a = gw
        .post(
            "alpha",
            &json!({"text": "check the Q3 numbers", "key": "k-1"}),
        )
        .await;
    assert_eq!(a.status, 201, "{}", a.body);
    assert_eq!(a.json()["id"], "k-1");
    assert_eq!(a.json()["repeat"], false);
    let lines = record_lines(&f.state("alpha"));
    assert_eq!(
        lines
            .iter()
            .filter(|l| l["kind"] == "stimulus.accepted" && l["item"]["id"] == "k-1")
            .count(),
        1
    );
    assert_eq!(fake.hits.load(Ordering::SeqCst), 1);
    // the list, from the record, shows it
    let l = gw.get("alpha", "queue").await;
    assert_eq!(l.status, 200, "{}", l.body);
    let v = l.json();
    assert_eq!(v["items"][0]["id"], "k-1");
    assert_eq!(v["items"][0]["text"], "check the Q3 numbers");
    assert_eq!(v["items"][0]["state"], "waiting");
    assert_eq!(v["items"][0]["role"], "owner");
}

#[tokio::test]
async fn the_same_key_twice_is_one_item_and_the_retry_says_so() {
    let mut f = Fleet::new("s1gw-key");
    let _fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let body = json!({"text": "once", "key": "k-once"});
    let a = gw.post("alpha", &body).await;
    let b = gw.post("alpha", &body).await;
    assert_eq!((a.status, b.status), (201, 200));
    assert_eq!(b.json()["id"], "k-once");
    assert_eq!(b.json()["repeat"], true);
    let n = record_lines(&f.state("alpha"))
        .iter()
        .filter(|l| l["kind"] == "stimulus.accepted")
        .count();
    assert_eq!(n, 1);
    assert_eq!(gw.get("alpha", "queue").await.json()["total"], 1);
}

#[tokio::test]
async fn a_stopped_instance_is_refused_with_start_it_first_and_nothing_is_written() {
    let f = Fleet::new("s1gw-stopped");
    // listed with an address, but no host holds the lock
    let cfg = f.config("alpha", Some("S1GW_OWNER_ALPHA"));
    unsafe { std::env::set_var("S1GW_OWNER_ALPHA", OWNER_KEY) };
    let fake = fake_host(f.state("alpha"), Mode::Normal).await;
    f.add("alpha", Some(&cfg), Some(&fake.addr));
    f.record(
        "alpha",
        vec![
            ("host.start", json!({"config": {}})),
            ("halted", json!({"why": "stop"})),
        ],
    );
    let before = whole_record(&f, "alpha");
    let gw = f.serve().await;
    let a = gw
        .post("alpha", &json!({"text": "hello?", "key": "k-x"}))
        .await;
    assert_eq!(a.status, 409, "{}", a.body);
    assert!(
        a.json()["message"]
            .as_str()
            .unwrap()
            .contains("start it first")
    );
    assert_eq!(whole_record(&f, "alpha"), before, "nothing was written");
    assert_eq!(
        fake.hits.load(Ordering::SeqCst),
        0,
        "no request went upstream"
    );
    // a down instance (no halt line) is refused the same way
    f.record("alpha", vec![("host.start", json!({"config": {}}))]);
    let a = gw.post("alpha", &json!({"text": "hello?"})).await;
    assert_eq!(a.status, 409);
    assert!(
        a.json()["message"]
            .as_str()
            .unwrap()
            .contains("start it first")
    );
}

#[tokio::test]
async fn the_read_only_token_may_read_the_queue_but_cannot_write() {
    let mut f = Fleet::new("s1gw-ro");
    let fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let w = send(
        client()
            .post(gw.url("alpha", "queue"))
            .bearer_auth(READ_TOKEN)
            .header("content-type", "application/json")
            .body(json!({"text": "x"}).to_string()),
    )
    .await;
    assert_eq!(w.status, 403, "{}", w.body);
    assert_eq!(w.json()["error"], "read_only");
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0);
    assert!(record_lines(&f.state("alpha")).is_empty());
    let r = send(
        client()
            .get(gw.url("alpha", "queue"))
            .bearer_auth(READ_TOKEN),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    // a token nobody holds is 401, on a read as on a write
    let r = send(client().get(gw.url("alpha", "queue")).bearer_auth("nope")).await;
    assert_eq!(r.status, 401);
}

#[tokio::test]
async fn the_owner_key_never_appears_in_anything_the_gateway_says() {
    let mut f = Fleet::new("s1gw-key-secret");
    let fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let mut wire = String::new();
    let mut take = |a: Answer| wire.push_str(&format!("{} {}\n", a.status, a.body));
    take(
        gw.post("alpha", &json!({"text": "fine", "key": "k-1"}))
            .await,
    );
    take(
        gw.post("alpha", &json!({"text": "fine", "key": "k-1"}))
            .await,
    );
    take(gw.get("alpha", "queue").await);
    take(gw.get("alpha", "summary").await);
    take(gw.get("alpha", "record").await);
    take(send(client().get(format!("{}/api/instances", gw.base))).await);
    take(gw.post("alpha", &json!({"text": ""})).await);
    take(send(client().delete(gw.url("alpha", "queue/k-1"))).await);
    take(send(client().put(gw.url("alpha", "queue"))).await);
    // a host that answers with the bearer it was given (a fault in the host)
    let mut f2 = Fleet::new("s1gw-key-echo");
    let _echo = running(&mut f2, "beta", Mode::EchoKey).await;
    let gw2 = f2.serve().await;
    let e = gw2.post("beta", &json!({"text": "x"})).await;
    assert_eq!(e.status, 500);
    take(e);
    // an item whose text holds a shaped secret and the key itself
    take(
        gw.post(
            "alpha",
            &json!({"text": format!("{SHAPED} and {OWNER_KEY}"), "key": "k-2"}),
        )
        .await,
    );
    take(gw.get("alpha", "queue?state=all").await);
    assert!(!wire.contains(OWNER_KEY), "the owner key reached an answer");
    assert!(!wire.contains(SHAPED), "a shaped secret reached an answer");
    assert!(wire.contains("[redacted]"));
    // the host was given the key, as a bearer, and only the key
    assert!(
        fake.seen
            .lock()
            .unwrap()
            .iter()
            .all(|(_, h)| !h.contains(READ_TOKEN))
    );
    // and the key is nowhere in the gateway's own files: none are written
    assert!(!std::fs::read_dir(f.registry()).unwrap().any(|e| {
        std::fs::read_to_string(e.unwrap().path())
            .unwrap()
            .contains(OWNER_KEY)
    }));
}

#[tokio::test]
async fn an_unreachable_host_is_a_502_in_words_and_the_gateway_stays_up() {
    let mut f = Fleet::new("s1gw-down");
    unsafe { std::env::set_var("S1GW_OWNER_ALPHA", OWNER_KEY) };
    let cfg = f.config("alpha", Some("S1GW_OWNER_ALPHA"));
    // an address nothing listens on
    let dead = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().to_string()
    };
    f.add("alpha", Some(&cfg), Some(&dead));
    f.twin("alpha", &dead, OWNER_KEY);
    f.hold("alpha");
    let gw = f.serve().await;
    let a = gw
        .post("alpha", &json!({"text": "hello", "key": "k-1"}))
        .await;
    assert_eq!(a.status, 502, "{}", a.body);
    assert_eq!(a.json()["error"], "instance_unreachable");
    let m = a.json()["message"].as_str().unwrap().to_string();
    assert!(m.contains("could not be reached"), "{m}");
    assert!(!m.contains(OWNER_KEY));
    // still answering
    assert_eq!(
        send(client().get(format!("{}/api/health", gw.base)))
            .await
            .status,
        200
    );
    assert_eq!(gw.get("alpha", "queue").await.status, 200);
}

#[tokio::test]
async fn a_host_that_never_answers_is_a_503_in_words_within_the_wait() {
    let mut f = Fleet::new("s1gw-silent");
    let _fake = running(&mut f, "alpha", Mode::Silent).await;
    let gw = f
        .serve_tuned(Tuning {
            upstream_wait: Duration::from_millis(300),
            ..Tuning::default()
        })
        .await;
    let t = std::time::Instant::now();
    let a = gw
        .post("alpha", &json!({"text": "hello", "key": "k-1"}))
        .await;
    assert!(t.elapsed() < Duration::from_secs(10));
    assert_eq!(a.status, 503, "{}", a.body);
    assert_eq!(a.json()["error"], "instance_not_answering");
    let m = a.json()["message"].as_str().unwrap().to_string();
    assert!(m.contains("same key"), "{m}");
    // not hung: other requests are served while it was waiting and after
    assert_eq!(
        send(client().get(format!("{}/api/health", gw.base)))
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn an_instance_the_gateway_config_does_not_name_gets_no_write_whatever_its_own_config_says() {
    let mut f = Fleet::new("s1gw-nokey");
    // the host's own config names a variable this process holds; the gateway's
    // config names nothing for this id
    unsafe { std::env::set_var("S1GW_HOSTCFG_ALPHA", PLANTED) };
    let cfg = f.config("alpha", Some("S1GW_HOSTCFG_ALPHA"));
    let fake = fake_host(f.state("alpha"), Mode::Normal).await;
    f.add("alpha", Some(&cfg), Some(&fake.addr));
    f.hold("alpha");
    let gw = f.serve().await;
    let a = gw.post("alpha", &json!({"text": "hello"})).await;
    assert_eq!(a.status, 503, "{}", a.body);
    assert_eq!(a.json()["error"], "no_owner_key");
    assert!(a.json()["message"].as_str().unwrap().contains("key_env"));
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0, "nothing was sent");
    assert!(!a.body.contains(PLANTED));
}

#[tokio::test]
async fn a_variable_an_instance_config_names_is_never_read_or_sent() {
    let mut f = Fleet::new("s1gw-planted");
    let fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let a = gw
        .post("alpha", &json!({"text": "hello", "key": "k-1"}))
        .await;
    assert_eq!(a.status, 201, "{}", a.body);
    let seen = fake.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    // the host got the gateway's own key and nothing the instance's config named
    assert!(seen[0].1.contains("authorization"));
    assert!(
        !seen[0].1.contains(PLANTED),
        "the config-named variable was sent"
    );
    assert_eq!(fake.last_bearer.lock().unwrap().as_deref(), Some(OWNER_KEY));
}

#[tokio::test]
async fn a_registered_address_that_is_not_private_is_refused_and_no_key_goes_to_it() {
    let mut f = Fleet::new("s1gw-public");
    let cfg = f.config("alpha", None);
    // a public address: nothing may be sent to it, with or without a key
    f.add("alpha", Some(&cfg), Some("203.0.113.9:7001"));
    f.twin("alpha", "203.0.113.9:7001", OWNER_KEY);
    f.hold("alpha");
    let gw = f.serve().await;
    let t = std::time::Instant::now();
    let a = gw.post("alpha", &json!({"text": "hello"})).await;
    assert!(t.elapsed() < Duration::from_secs(5), "refused, not tried");
    assert_eq!(a.status, 503, "{}", a.body);
    assert_eq!(a.json()["error"], "no_address");
    assert!(a.json()["message"].as_str().unwrap().contains("private"));
}

#[tokio::test]
async fn a_host_and_origin_the_gateway_does_not_answer_for_are_refused_before_the_host() {
    let mut f = Fleet::new("s1gw-guard");
    let fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let body = json!({"text": "x"}).to_string();
    let r = send(
        client()
            .post(gw.url("alpha", "queue"))
            .header("origin", "http://evil.example")
            .header("content-type", "application/json")
            .body(body.clone()),
    )
    .await;
    assert_eq!(
        (r.status, r.json()["error"].as_str().map(str::to_string)),
        (403, Some("bad_origin".into()))
    );
    let r = send(
        client()
            .post(gw.url("alpha", "queue"))
            .header("host", "evil.example")
            .header("content-type", "application/json")
            .body(body),
    )
    .await;
    assert_eq!(
        (r.status, r.json()["error"].as_str().map(str::to_string)),
        (403, Some("bad_host".into()))
    );
    let r = send(
        client()
            .get(gw.url("alpha", "queue"))
            .header("host", "evil.example"),
    )
    .await;
    assert_eq!(r.status, 403);
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_200000_character_task_goes_through_and_is_listed_whole() {
    let mut f = Fleet::new("s1gw-whole");
    let _fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let text: String = (0..200_000)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    let a = gw
        .post("alpha", &json!({"text": text, "key": "k-big"}))
        .await;
    assert_eq!(a.status, 201);
    assert_eq!(a.json()["item"]["text"].as_str().unwrap(), text);
    let l = gw.get("alpha", "queue").await.json();
    assert_eq!(l["items"][0]["text"].as_str().unwrap(), text);
}

// ─── the list, from the record ───────────────────────────────────────────────

#[tokio::test]
async fn the_list_of_a_stopped_instance_is_read_from_its_record() {
    let f = Fleet::new("s1gw-list-stopped");
    f.add("alpha", None, None);
    f.record(
        "alpha",
        vec![
            ("host.start", json!({"config": {}})),
            accepted_line(
                "m-1",
                "owner",
                "owner",
                T0 + 1000,
                "please look at the build",
            ),
            accepted_line("m-2", "owner", "owner", T0 + 2000, "and the docs"),
            ("halted", json!({"why": "stop"})),
        ],
    );
    let gw = f.serve().await;
    let l = gw.get("alpha", "queue").await;
    assert_eq!(l.status, 200, "{}", l.body);
    let v = l.json();
    assert_eq!(v["total"], 2);
    let ids: Vec<&str> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["m-1", "m-2"]);
    assert_eq!(v["items"][0]["position"], 1);
    assert_eq!(v["items"][1]["position"], 2);
    assert_eq!(v["items"][0]["accepted_at"], T0 + 1000);
    assert!(v["redaction"]["env_unset"].is_array());
}

#[tokio::test]
async fn peer_items_are_never_listed_above_owner_items() {
    let f = Fleet::new("s1gw-order");
    f.add("alpha", None, None);
    f.record(
        "alpha",
        vec![
            ("host.start", json!({"config": {}})),
            accepted_line("p-1", "peer", "peer:atlas", T0 + 1000, "a note from atlas"),
            accepted_line("o-1", "owner", "owner", T0 + 2000, "owner first please"),
            accepted_line("p-2", "peer", "peer:borealis", T0 + 3000, "another note"),
            accepted_line("o-2", "owner", "owner", T0 + 4000, "owner second"),
        ],
    );
    let gw = f.serve().await;
    let v = gw.get("alpha", "queue").await.json();
    let who: Vec<(&str, &str)> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["id"].as_str().unwrap(), i["role"].as_str().unwrap()))
        .collect();
    assert_eq!(
        who,
        [
            ("o-1", "owner"),
            ("o-2", "owner"),
            ("p-1", "peer"),
            ("p-2", "peer")
        ]
    );
    // the peer is named
    assert_eq!(v["items"][2]["name"], "atlas");
    assert_eq!(v["items"][3]["name"], "borealis");
    assert!(v["items"][0]["name"].is_null());
}

#[tokio::test]
async fn the_states_the_desk_reasons_and_the_answered_follow_the_record() {
    let f = Fleet::new("s1gw-states");
    f.add("alpha", None, None);
    f.record(
        "alpha",
        vec![
            ("host.start", json!({"config": {}})),
            accepted_line("a-1", "owner", "owner", T0, "being answered"),
            accepted_line("a-2", "owner", "owner", T0, "withdrawn one"),
            accepted_line("a-3", "owner", "owner", T0, "answered one"),
            accepted_line("p-1", "peer", "peer:atlas", T0, "held at break"),
            accepted_line("p-2", "peer", "peer:atlas", T0, "held as digest"),
            ("stimulus.disposed", json!({"id": "a-2", "disposition": "withdrawn", "turn": 0})),
            ("stimulus.admitted", json!({"turn": 1, "ids": ["a-3"]})),
            ("stimulus.disposed", json!({"id": "a-3", "disposition": "answered", "turn": 1})),
            ("turn.ended", json!({"turn": 1, "status": "ok", "final_text": ""})),
            ("stimulus.admitted", json!({"turn": 2, "ids": ["a-1"]})),
            (
                "decision.admit",
                json!({"turn": 2, "boundary": 2, "choice": {"forms": {"p-1": "at_break", "p-2": "digest"}, "interrupts": 0}}),
            ),
        ],
    );
    let gw = f.serve().await;
    let v = gw.get("alpha", "queue?state=all").await.json();
    let by = |id: &str| -> Value {
        v["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(by("a-1")["state"], "answering");
    assert_eq!(by("a-1")["turn"], 2);
    assert_eq!(by("a-2")["state"], "withdrawn");
    assert_eq!(by("a-3")["state"], "answered");
    assert_eq!(by("a-3")["turn"], 1);
    assert_eq!(by("p-1")["state"], "waiting");
    assert!(by("p-1")["held"].as_str().unwrap().contains("next break"));
    assert!(by("p-2")["held"].as_str().unwrap().contains("turn header"));
    assert!(by("a-1")["held"].is_null());
    // the order of the whole list: answering, waiting, then what is over
    let ids: Vec<&str> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["a-1", "p-1", "p-2", "a-3", "a-2"]);
    assert_eq!(v["counts"]["waiting"], 2);
    assert_eq!(v["counts"]["answering"], 1);
    assert_eq!(v["counts"]["answered"], 1);
    assert_eq!(v["counts"]["withdrawn"], 1);
    // the default list is what is open
    let open = gw.get("alpha", "queue").await.json();
    assert_eq!(open["total"], 3);
}

#[tokio::test]
async fn an_unknown_instance_and_other_methods_are_refused() {
    let f = Fleet::new("s1gw-misc");
    f.add("alpha", None, None);
    let gw = f.serve().await;
    assert_eq!(gw.get("nope", "queue").await.status, 404);
    let d = send(client().delete(gw.url("alpha", "queue"))).await;
    assert_eq!(d.status, 405);
    // a stop door is not served: no Start door either
    let s = send(client().post(gw.url("alpha", "start"))).await;
    assert_eq!(s.status, 404);
}

#[tokio::test]
async fn an_id_the_gateway_config_also_names_is_written_to_with_the_configured_key() {
    let mut f = Fleet::new("s1gw-twin");
    let fake = fake_host(f.state("alpha"), Mode::Normal).await;
    // the registry holds no address and no config: only the gateway config knows
    f.add("alpha", None, None);
    f.hold("alpha");
    let mut s = f.settings();
    s.instances = vec![rung_gateway::Instance {
        id: "alpha".into(),
        name: "Alpha".into(),
        url: format!("http://{}", fake.addr),
        key: OWNER_KEY.into(),
    }];
    let running = rung_gateway::start(s).await.unwrap();
    let base = format!("http://{}", running.addr);
    let a = send(
        client()
            .post(format!("{base}/api/i/alpha/v1/queue"))
            .header("content-type", "application/json")
            .body(json!({"text": "via the configured key", "key": "k-twin"}).to_string()),
    )
    .await;
    assert_eq!(a.status, 201, "{}", a.body);
    assert_eq!(fake.hits.load(Ordering::SeqCst), 1);
    assert!(!a.body.contains(OWNER_KEY));
    running.shutdown();
}

#[tokio::test]
async fn an_owner_key_a_header_cannot_carry_is_refused_in_words_and_never_sent() {
    let mut f = Fleet::new("s1gw-badkey");
    let cfg = f.config("alpha", None);
    let fake = fake_host(f.state("alpha"), Mode::Normal).await;
    f.add("alpha", Some(&cfg), Some(&fake.addr));
    f.twin("alpha", &fake.addr, "kéy-with-non-ascii-0123456789");
    f.hold("alpha");
    let gw = f.serve().await;
    let a = gw.post("alpha", &json!({"text": "hello"})).await;
    assert_eq!(a.status, 503, "{}", a.body);
    assert_eq!(a.json()["error"], "no_owner_key");
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0);
}

// ─── who a request is: the Authorization header ──────────────────────────────

/// What a present `Authorization` header that is not a read-only token does: it
/// is refused 401, never read as "no token" (which is the owner's). No header at
/// all stays the owner's: the tailnet is the boundary.
#[tokio::test]
async fn an_authorization_header_that_is_not_a_known_bearer_is_401_on_every_queue_door() {
    use reqwest::header::{AUTHORIZATION, HeaderValue};
    let mut f = Fleet::new("s1gw-auth-forms");
    let fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let hv = |b: &[u8]| HeaderValue::from_bytes(b).unwrap();
    let ro = format!("Bearer {READ_TOKEN}");
    let host_key = format!("Bearer {OWNER_KEY}");
    // (label, headers, status on the list, status on an add)
    let forms: Vec<(&str, Vec<HeaderValue>, u16, u16)> = vec![
        ("none", vec![], 200, 201),
        ("bare Bearer", vec![hv(b"Bearer")], 401, 401),
        ("empty Bearer", vec![hv(b"Bearer ")], 401, 401),
        ("Basic", vec![hv(b"Basic Zm9vOmJhcg==")], 401, 401),
        ("garbage", vec![hv(b"garbage")], 401, 401),
        ("not UTF-8", vec![hv(b"Bearer \xff\xfe")], 401, 401),
        ("a token nobody holds", vec![hv(b"Bearer nobody")], 401, 401),
        (
            "the host's own key",
            vec![hv(host_key.as_bytes())],
            401,
            401,
        ),
        (
            "a token that is one byte off",
            vec![hv(format!("{ro}x").as_bytes())],
            401,
            401,
        ),
        (
            "lower-case scheme, right token",
            vec![hv(format!("bearer {READ_TOKEN}").as_bytes())],
            200,
            403,
        ),
        ("read-only token", vec![hv(ro.as_bytes())], 200, 403),
        (
            "read-only and garbage",
            vec![hv(ro.as_bytes()), hv(b"garbage")],
            401,
            401,
        ),
    ];
    let mut n = 0;
    for (label, headers, want_list, want_add) in forms {
        n += 1;
        let mut list = client().get(gw.url("alpha", "queue"));
        let mut add = client()
            .post(gw.url("alpha", "queue"))
            .header("content-type", "application/json")
            .body(json!({"text": "x", "key": format!("k-form-{n}")}).to_string());
        for h in &headers {
            list = list.header(AUTHORIZATION, h.clone());
            add = add.header(AUTHORIZATION, h.clone());
        }
        let l = send(list).await;
        let a = send(add).await;
        assert_eq!(l.status, want_list, "list: {label}");
        assert_eq!(a.status, want_add, "add: {label}");
    }
    // only the owner's add reached the host, and only it was written
    assert_eq!(fake.hits.load(Ordering::SeqCst), 1);
    let wrote = record_lines(&f.state("alpha"))
        .iter()
        .filter(|l| l["kind"] == "stimulus.accepted")
        .count();
    assert_eq!(wrote, 1);
    // the query form: an empty token is refused the same way
    let l = send(client().get(format!("{}?token=", gw.url("alpha", "queue")))).await;
    assert_eq!(l.status, 401);
}

// ─── a long upload is not a stall ────────────────────────────────────────────

/// A client that is still sending is not a host that does not answer: the wait
/// for the host counts from the last byte moved, not from the first.
#[tokio::test]
async fn an_upload_slower_than_the_wait_is_not_cut_while_it_keeps_moving() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut f = Fleet::new("s1gw-slow-upload");
    let _fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f
        .serve_tuned(Tuning {
            upstream_wait: Duration::from_millis(400),
            ..Tuning::default()
        })
        .await;
    let body = json!({"text": "t".repeat(3000), "key": "k-slow"}).to_string();
    let addr = gw.base.trim_start_matches("http://").to_string();
    let mut s = tokio::net::TcpStream::connect(&addr).await.unwrap();
    let head = format!(
        "POST /api/i/alpha/v1/queue HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).await.unwrap();
    // five pieces, 150 ms apart: 750 ms in all, past the 400 ms wait
    let piece = body.len().div_ceil(5);
    for chunk in body.as_bytes().chunks(piece) {
        s.write_all(chunk).await.unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    assert!(
        out.starts_with("HTTP/1.1 201"),
        "{}",
        &out[..out.len().min(200)]
    );
}

// ─── S3: move and cancel through the gateway ─────────────────────────────────

impl Gw {
    async fn mv(&self, id: &str, item: &str, body: &Value) -> Answer {
        send(
            client()
                .post(self.url(id, &format!("queue/{item}/move")))
                .header("content-type", "application/json")
                .body(body.to_string()),
        )
        .await
    }

    async fn cancel(&self, id: &str, item: &str) -> Answer {
        send(client().delete(self.url(id, &format!("queue/{item}")))).await
    }
}

#[tokio::test]
async fn a_move_and_a_cancel_are_forwarded_to_the_running_host_with_the_gateways_key() {
    let mut f = Fleet::new("s3gw-forward");
    let fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let body = json!({"before": "k-2", "key": "mv-1"});
    let a = gw.mv("alpha", "k-1", &body).await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.json()["done"], "/v1/queue/k-1/move");
    let c = gw.cancel("alpha", "k-2").await;
    assert_eq!(c.status, 200, "{}", c.body);
    assert_eq!(c.json()["done"], "/v1/queue/k-2");
    // an id as the client wrote it, percent-encoded, goes on as it came
    let c = gw.cancel("alpha", "a%20file").await;
    assert_eq!(c.status, 200, "{}", c.body);
    assert_eq!(c.json()["done"], "/v1/queue/a%20file");
    let seen: Vec<String> = fake
        .seen
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.0.clone())
        .collect();
    assert_eq!(
        seen,
        [
            "POST /v1/queue/k-1/move",
            "DELETE /v1/queue/k-2",
            "DELETE /v1/queue/a%20file"
        ]
    );
    // the move's body went on whole, and the host was given the gateway's key
    let bodies = fake.bodies.lock().unwrap().clone();
    assert_eq!(serde_json::from_slice::<Value>(&bodies[0]).unwrap(), body);
    assert!(bodies[1].is_empty());
    assert_eq!(fake.last_bearer.lock().unwrap().as_deref(), Some(OWNER_KEY));
    assert!(
        fake.seen
            .lock()
            .unwrap()
            .iter()
            .all(|(_, h)| !h.contains(READ_TOKEN))
    );
}

#[tokio::test]
async fn the_read_only_token_cannot_move_or_cancel_and_nothing_goes_upstream() {
    let mut f = Fleet::new("s3gw-ro");
    let fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let m = send(
        client()
            .post(gw.url("alpha", "queue/k-1/move"))
            .bearer_auth(READ_TOKEN)
            .header("content-type", "application/json")
            .body(json!({"before": null}).to_string()),
    )
    .await;
    assert_eq!(m.status, 403, "{}", m.body);
    assert_eq!(m.json()["error"], "read_only");
    let c = send(
        client()
            .delete(gw.url("alpha", "queue/k-1"))
            .bearer_auth(READ_TOKEN),
    )
    .await;
    assert_eq!(c.status, 403, "{}", c.body);
    assert_eq!(c.json()["error"], "read_only");
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0);
    // a token nobody holds is 401 on both
    let c = send(
        client()
            .delete(gw.url("alpha", "queue/k-1"))
            .bearer_auth("nope"),
    )
    .await;
    assert_eq!(c.status, 401);
    let m = send(
        client()
            .post(gw.url("alpha", "queue/k-1/move"))
            .bearer_auth("nope")
            .header("content-type", "application/json")
            .body("{}"),
    )
    .await;
    assert_eq!(m.status, 401);
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_stopped_instance_refuses_a_move_and_a_cancel_with_start_it_first() {
    let f = Fleet::new("s3gw-stopped");
    let cfg = f.config("alpha", Some("S3GW_OWNER_ALPHA"));
    unsafe { std::env::set_var("S3GW_OWNER_ALPHA", OWNER_KEY) };
    let fake = fake_host(f.state("alpha"), Mode::Normal).await;
    f.add("alpha", Some(&cfg), Some(&fake.addr));
    f.record(
        "alpha",
        vec![
            ("host.start", json!({"config": {}})),
            accepted_line("k-1", "owner", "owner", T0, "one"),
            ("halted", json!({"why": "stop"})),
        ],
    );
    let before = whole_record(&f, "alpha");
    let gw = f.serve().await;
    for a in [
        gw.mv("alpha", "k-1", &json!({"before": null})).await,
        gw.cancel("alpha", "k-1").await,
    ] {
        assert_eq!(a.status, 409, "{}", a.body);
        assert_eq!(a.json()["error"], "not_running");
        assert!(
            a.json()["message"]
                .as_str()
                .unwrap()
                .contains("start it first")
        );
    }
    assert_eq!(whole_record(&f, "alpha"), before, "nothing was written");
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0, "nothing went upstream");
}

#[tokio::test]
async fn an_instance_the_gateway_holds_no_key_for_gets_no_move_or_cancel() {
    let mut f = Fleet::new("s3gw-nokey");
    unsafe { std::env::set_var("S3GW_HOSTCFG_BETA", PLANTED) };
    let cfg = f.config("beta", Some("S3GW_HOSTCFG_BETA"));
    let fake = fake_host(f.state("beta"), Mode::Normal).await;
    f.add("beta", Some(&cfg), Some(&fake.addr));
    f.hold("beta");
    let gw = f.serve().await;
    for a in [
        gw.mv("beta", "k-1", &json!({"before": null})).await,
        gw.cancel("beta", "k-1").await,
    ] {
        assert_eq!(a.status, 503, "{}", a.body);
        assert_eq!(a.json()["error"], "no_owner_key");
    }
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn the_gateways_key_is_in_no_answer_to_a_move_or_a_cancel() {
    let mut f = Fleet::new("s3gw-echo");
    let _echo = running(&mut f, "beta", Mode::EchoKey).await;
    let gw = f.serve().await;
    let mut wire = String::new();
    for a in [
        gw.mv("beta", "k-1", &json!({"before": null})).await,
        gw.cancel("beta", "k-1").await,
    ] {
        assert_eq!(a.status, 500);
        wire.push_str(&a.body);
    }
    assert!(!wire.contains(OWNER_KEY), "the key reached an answer");
    assert!(wire.contains("[redacted]"));
}

#[tokio::test]
async fn a_bad_host_or_origin_is_refused_before_the_host_on_a_move_and_a_cancel() {
    let mut f = Fleet::new("s3gw-guard");
    let fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    for rb in [
        client()
            .post(gw.url("alpha", "queue/k-1/move"))
            .header("origin", "http://evil.example")
            .header("content-type", "application/json")
            .body("{}"),
        client()
            .delete(gw.url("alpha", "queue/k-1"))
            .header("origin", "http://evil.example"),
        client()
            .delete(gw.url("alpha", "queue/k-1"))
            .header("host", "evil.example"),
    ] {
        let r = send(rb).await;
        assert_eq!(r.status, 403, "{}", r.body);
    }
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn the_other_methods_and_paths_under_the_queue_are_refused_by_the_gateway() {
    let mut f = Fleet::new("s3gw-methods");
    let fake = running(&mut f, "alpha", Mode::Normal).await;
    let gw = f.serve().await;
    let st = |rb: reqwest::RequestBuilder| async { send(rb).await.status };
    assert_eq!(st(client().get(gw.url("alpha", "queue/k-1"))).await, 405);
    assert_eq!(st(client().put(gw.url("alpha", "queue/k-1"))).await, 405);
    assert_eq!(st(client().post(gw.url("alpha", "queue/k-1"))).await, 405);
    assert_eq!(
        st(client().get(gw.url("alpha", "queue/k-1/move"))).await,
        405
    );
    assert_eq!(
        st(client().delete(gw.url("alpha", "queue/k-1/move"))).await,
        405
    );
    assert_eq!(st(client().delete(gw.url("alpha", "queue"))).await, 405);
    assert_eq!(
        st(client().delete(gw.url("alpha", "queue/a/b/c"))).await,
        404
    );
    assert_eq!(st(client().delete(gw.url("alpha", "queue/%ff"))).await, 404);
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_unreachable_or_silent_host_is_a_plain_failure_for_a_move_and_a_cancel() {
    let mut f = Fleet::new("s3gw-dead");
    let cfg = f.config("alpha", Some("S3GW_OWNER_DEAD"));
    unsafe { std::env::set_var("S3GW_OWNER_DEAD", OWNER_KEY) };
    let dead = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().to_string()
    };
    f.add("alpha", Some(&cfg), Some(&dead));
    f.twin("alpha", &dead, OWNER_KEY);
    f.hold("alpha");
    let _silent = running(&mut f, "beta", Mode::Silent).await;
    let gw = f
        .serve_tuned(Tuning {
            upstream_wait: Duration::from_millis(300),
            ..Tuning::default()
        })
        .await;
    let a = gw.cancel("alpha", "k-1").await;
    assert_eq!(a.status, 502, "{}", a.body);
    let m = a.json()["message"].as_str().unwrap().to_string();
    assert!(m.contains("nothing was changed"), "{m}");
    let b = gw.mv("beta", "k-1", &json!({"before": null})).await;
    assert_eq!(b.status, 503, "{}", b.body);
    assert_eq!(b.json()["error"], "instance_not_answering");
    assert!(
        b.json()["message"]
            .as_str()
            .unwrap()
            .contains("the change may have been made")
    );
}

#[tokio::test]
async fn the_list_shows_the_folded_order_and_a_200000_character_item_whole() {
    let f = Fleet::new("s3gw-order");
    f.add("alpha", None, None);
    let big: String = (0..200_000)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    f.record(
        "alpha",
        vec![
            ("host.start", json!({"config": {}})),
            accepted_line("o-1", "owner", "owner", T0, "one"),
            accepted_line("o-2", "owner", "owner", T0, &big),
            accepted_line("o-3", "owner", "owner", T0, "three"),
            accepted_line("p-1", "peer", "peer:atlas", T0, "from a peer"),
            ("queue.moved", json!({"id": "o-2", "before": "o-1"})),
            (
                "queue.moved",
                json!({"id": "o-3", "before": "o-2", "key": "mv-1"}),
            ),
            // a peer's item cannot be put above an owner's, whatever a line says
            ("queue.moved", json!({"id": "p-1", "before": "o-3"})),
            ("queue.moved", json!({"id": "o-1", "before": "p-1"})),
        ],
    );
    let gw = f.serve().await;
    let v = gw.get("alpha", "queue").await.json();
    let ids: Vec<&str> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["o-3", "o-2", "o-1", "p-1"]);
    assert_eq!(v["items"][1]["position"], 2);
    assert!(v["items"][1]["text"].as_str().unwrap() == big, "whole");
    // cancelled, it is listed whole among what is over
    f.record(
        "alpha",
        vec![
            ("host.start", json!({"config": {}})),
            accepted_line("o-2", "owner", "owner", T0, &big),
            (
                "stimulus.disposed",
                json!({"id": "o-2", "disposition": "withdrawn", "turn": 0}),
            ),
        ],
    );
    let v = gw.get("alpha", "queue?state=withdrawn").await.json();
    assert_eq!(v["items"][0]["state"], "withdrawn");
    assert!(v["items"][0]["text"].as_str().unwrap() == big, "whole");
}
