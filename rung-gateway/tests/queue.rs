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
const SHAPED: &str = "sk-SENTINELs1gwshaped0123456789abcdef";
const T0: i64 = 1_791_208_186_000;

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

// ─── the world ───────────────────────────────────────────────────────────────

struct Fleet {
    root: TempDir,
    locks: Vec<StateLock>,
}

impl Fleet {
    fn new(tag: &str) -> Self {
        let root = TempDir::new(tag);
        std::fs::create_dir_all(root.path().join("registry")).unwrap();
        Fleet {
            root,
            locks: Vec::new(),
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
            instances: vec![],
        }
    }

    async fn serve(&self) -> Gw {
        self.serve_tuned(Tuning::default()).await
    }

    async fn serve_tuned(&self, tuning: Tuning) -> Gw {
        let running =
            rung_gateway::start_tuned(self.settings(), Arc::new(StaticRegistry(vec![])), tuning)
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
    let (h2, s2) = (hits.clone(), seen.clone());
    tokio::spawn(async move {
        loop {
            let (sock, _) = l.accept().await.unwrap();
            let (state, hits, seen) = (state.clone(), h2.clone(), s2.clone());
            tokio::spawn(async move {
                let svc = hyper::service::service_fn(move |req: Request<hyper::body::Incoming>| {
                    let (state, hits, seen) = (state.clone(), hits.clone(), seen.clone());
                    async move {
                        hits.fetch_add(1, Ordering::SeqCst);
                        let bearer = req
                            .headers()
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("")
                            .to_string();
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
                        let body = req.into_body().collect().await.unwrap().to_bytes();
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
    FakeHost { addr, hits, seen }
}

/// A running instance `id`: its owner token named by a variable this process
/// holds, its lock held, its listener a fake.
async fn running(f: &mut Fleet, id: &str, mode: Mode) -> FakeHost {
    let env = format!("S1GW_OWNER_{}", id.to_uppercase());
    unsafe { std::env::set_var(&env, OWNER_KEY) };
    let cfg = f.config(id, Some(&env));
    let fake = fake_host(f.state(id), mode).await;
    f.add(id, Some(&cfg), Some(&fake.addr));
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
async fn an_instance_the_gateway_holds_no_owner_key_for_is_refused_in_words() {
    let mut f = Fleet::new("s1gw-nokey");
    let cfg = f.config("alpha", None);
    let fake = fake_host(f.state("alpha"), Mode::Normal).await;
    f.add("alpha", Some(&cfg), Some(&fake.addr));
    f.hold("alpha");
    let gw = f.serve().await;
    let a = gw.post("alpha", &json!({"text": "hello"})).await;
    assert_eq!(a.status, 503, "{}", a.body);
    assert_eq!(a.json()["error"], "no_owner_key");
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0);
    // the variable is named but not held by this process
    let cfg = f.config("alpha", Some("S1GW_NOT_SET_ANYWHERE"));
    f.add("alpha", Some(&cfg), Some(&fake.addr));
    let a = gw.post("alpha", &json!({"text": "hello"})).await;
    assert_eq!(a.status, 503);
    assert_eq!(a.json()["error"], "no_owner_key");
    assert_eq!(fake.hits.load(Ordering::SeqCst), 0);
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
