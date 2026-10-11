//! H5: the event stream door of the gateway, over real HTTP.
//!
//! `GET /api/i/{id}/v1/events` on a native instance replays the instance's
//! record from the client's cursor and then follows it, from the instance's
//! own state directory (so a stopped instance replays too), and relays the
//! partial frames of a running host's turn from the host's own door. A
//! "running" host is one whose state lock the test holds; the record is written
//! by hand; the host's door is a small fake that speaks its shape. A sentinel is
//! never put in an assertion message.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rung_gateway::Settings;
use rung_host::statelock::StateLock;
use rung_testkit::TempDir;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const READ_TOKEN: &str = "view-token-canary-h5-4412";
const SHAPED: &str = "sk-SENTINELh5gwshaped0123456789abcdef";
const PLAIN: &str = "plainsentinelh5gw-0123456789";
const DOOR_TOKEN: &str = "hosttokenh5gw-aaaaaaaaaaaa";
const T0: i64 = 1_791_208_186_000;
const GUARD: Duration = Duration::from_secs(20);

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

struct Fleet {
    root: TempDir,
    locks: Vec<StateLock>,
    next: std::collections::BTreeMap<String, u64>,
}

impl Fleet {
    fn new(tag: &str) -> Self {
        let root = TempDir::new(tag);
        std::fs::create_dir_all(root.path().join("registry")).unwrap();
        Fleet {
            root,
            locks: Vec::new(),
            next: Default::default(),
        }
    }

    fn registry(&self) -> PathBuf {
        self.root.path().join("registry")
    }

    fn state(&self, id: &str) -> PathBuf {
        self.root.path().join(id).join("state")
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

    fn push(&mut self, id: &str, kind: &str, body: Value) -> u64 {
        let n = self.next.entry(id.to_string()).or_insert(1);
        let mut m = body.as_object().cloned().unwrap_or_default();
        m.insert("seq".into(), (*n).into());
        m.insert("at".into(), (T0 + *n as i64).into());
        m.insert("kind".into(), kind.into());
        let seq = *n;
        *n += 1;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.state(id).join("record/seg-00000001.ndjson"))
            .unwrap();
        writeln!(f, "{}", Value::Object(m)).unwrap();
        seq
    }

    fn push_n(&mut self, id: &str, n: u64) {
        for i in 0..n {
            self.push(id, "note.written", json!({"text": format!("n{i}")}));
        }
    }

    fn hold(&mut self, id: &str) {
        self.locks
            .push(StateLock::acquire(&self.state(id)).expect("lock"));
    }

    fn release(&mut self) {
        self.locks.clear();
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
        let running = rung_gateway::start(self.settings()).await.unwrap();
        Gw {
            base: format!("http://{}", running.addr),
            addr: running.addr,
            _running: running,
        }
    }
}

struct Gw {
    base: String,
    addr: std::net::SocketAddr,
    _running: rung_gateway::Running,
}

/// One server-sent frame.
#[derive(Debug, Clone)]
struct Sse {
    event: String,
    id: Option<String>,
    data: Value,
}

struct Stream {
    resp: reqwest::Response,
    buf: String,
}

impl Stream {
    async fn open(
        gw: &Gw,
        path: &str,
        rb: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    ) -> Stream {
        let r = rb(client().get(format!("{}{path}", gw.base)));
        let resp = tokio::time::timeout(GUARD, r.send())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resp.status(), 200, "{path}");
        Stream {
            resp,
            buf: String::new(),
        }
    }

    async fn next(&mut self) -> Option<Sse> {
        loop {
            if let Some(i) = self.buf.find("\n\n") {
                let block: String = self.buf.drain(..i + 2).collect();
                let (mut event, mut id, mut data) = (String::new(), None, String::new());
                for l in block.lines() {
                    if let Some(v) = l.strip_prefix("event: ") {
                        event = v.into();
                    } else if let Some(v) = l.strip_prefix("id: ") {
                        id = Some(v.to_string());
                    } else if let Some(v) = l.strip_prefix("data: ") {
                        data.push_str(v);
                    }
                }
                if event.is_empty() {
                    continue;
                }
                return Some(Sse {
                    event,
                    id,
                    data: serde_json::from_str(&data).unwrap(),
                });
            }
            let chunk = tokio::time::timeout(GUARD, self.resp.chunk())
                .await
                .expect("the stream hung")
                .unwrap()?;
            self.buf.push_str(std::str::from_utf8(&chunk).unwrap());
        }
    }

    async fn records(&mut self, n: usize) -> Vec<u64> {
        let mut out = Vec::new();
        while out.len() < n {
            let f = self.next().await.expect("the stream ended");
            if f.event == "record" {
                assert_eq!(f.id.as_deref(), Some(f.data["seq"].to_string().as_str()));
                out.push(f.data["seq"].as_u64().unwrap());
            }
        }
        out
    }

    async fn rest(&mut self) -> Vec<Sse> {
        let mut v = Vec::new();
        while let Some(f) = self.next().await {
            v.push(f);
        }
        v
    }
}

async fn answer(rb: reqwest::RequestBuilder) -> (u16, Value) {
    let r = rb.send().await.unwrap();
    let s = r.status().as_u16();
    (s, r.json().await.unwrap_or(Value::Null))
}

// ─── replay and live ─────────────────────────────────────────────────────────

#[tokio::test]
async fn replay_then_live_with_no_gap_and_no_repeat_across_the_seam() {
    let mut f = Fleet::new("h5gw-live");
    f.add("alpha", None, None);
    f.push_n("alpha", 5);
    f.hold("alpha");
    let gw = f.serve().await;
    let mut s = Stream::open(&gw, "/api/i/alpha/v1/events", |r| r).await;
    let open = s.next().await.unwrap();
    assert_eq!(open.event, "open");
    assert_eq!(open.data["contract"], 1);
    assert_eq!(open.data["live"], true);
    assert_eq!(s.records(2).await, [1, 2]);
    // a line lands in the middle of the replay
    f.push_n("alpha", 1);
    assert_eq!(s.records(4).await, [3, 4, 5, 6]);
    // and live lines follow, once each, in order
    f.push_n("alpha", 3);
    assert_eq!(s.records(3).await, [7, 8, 9]);
}

#[tokio::test]
async fn a_reconnect_resumes_from_the_cursor_by_query_or_last_event_id() {
    let mut f = Fleet::new("h5gw-cursor");
    f.add("alpha", None, None);
    f.push_n("alpha", 8);
    f.hold("alpha");
    let gw = f.serve().await;
    let mut a = Stream::open(&gw, "/api/i/alpha/v1/events?after=5", |r| r).await;
    assert_eq!(a.records(3).await, [6, 7, 8]);
    // the header names the last line seen; it beats the address's first cursor
    let mut b = Stream::open(&gw, "/api/i/alpha/v1/events?after=1", |r| {
        r.header("last-event-id", "6")
    })
    .await;
    assert_eq!(b.records(2).await, [7, 8]);
    f.push_n("alpha", 1);
    assert_eq!(b.records(1).await, [9]);
}

#[tokio::test]
async fn refusals_come_before_the_stream() {
    let mut f = Fleet::new("h5gw-refuse");
    f.add("alpha", None, None);
    f.push_n("alpha", 2);
    f.hold("alpha");
    let gw = f.serve().await;
    let get = |p: &str| client().get(format!("{}{p}", gw.base));
    let (s, b) = answer(get("/api/i/alpha/v1/events?after=7")).await;
    assert_eq!((s, b["error"].as_str()), (409, Some("cursor_ahead")));
    for q in [
        "after=x",
        "after=1&after=2",
        "partials=never",
        "nope=1",
        "offset=1",
    ] {
        let (s, b) = answer(get(&format!("/api/i/alpha/v1/events?{q}"))).await;
        assert_eq!((s, b["error"].as_str()), (400, Some("bad_request")), "{q}");
    }
    // HEAD: the headers of a stream, and no body
    let head = client()
        .head(format!("{}/api/i/alpha/v1/events", gw.base))
        .send()
        .await
        .unwrap();
    assert_eq!(head.status(), 200);
    assert_eq!(head.headers()["content-type"], "text/event-stream");
    let (s, b) = answer(get("/api/i/nobody/v1/events")).await;
    assert_eq!((s, b["error"].as_str()), (404, Some("no_such_instance")));
    std::fs::remove_dir_all(f.state("alpha")).unwrap();
    let (s, b) = answer(get("/api/i/alpha/v1/events")).await;
    assert_eq!((s, b["error"].as_str()), (404, Some("state_dir_missing")));
}

#[tokio::test]
async fn a_stopped_instance_replays_its_record_and_the_stream_ends_cleanly() {
    let mut f = Fleet::new("h5gw-stopped");
    f.add("beta", None, None);
    f.push_n("beta", 3);
    f.push("beta", "halted", json!({"why": "stopped"}));
    let gw = f.serve().await;
    let mut s = Stream::open(&gw, "/api/i/beta/v1/events", |r| r).await;
    let frames = s.rest().await; // the body ends
    assert_eq!(frames[0].event, "open");
    assert_eq!(frames[0].data["live"], false);
    let recs: Vec<u64> = frames
        .iter()
        .filter(|x| x.event == "record")
        .map(|x| x.data["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(recs, [1, 2, 3, 4]);
    let end = frames.last().unwrap();
    assert_eq!(end.event, "end");
    assert_eq!(end.data["reason"], "stopped");
    assert_eq!(end.data["last_seq"], 4);
    // a host that stops while a client watches ends that stream too
    let mut g = Fleet::new("h5gw-stops-live");
    g.add("gamma", None, None);
    g.push_n("gamma", 2);
    g.hold("gamma");
    let gw2 = g.serve().await;
    let mut w = Stream::open(&gw2, "/api/i/gamma/v1/events", |r| r).await;
    assert_eq!(w.records(2).await, [1, 2]);
    g.push("gamma", "halted", json!({}));
    g.release();
    let tail = w.rest().await;
    assert_eq!(tail.last().unwrap().event, "end");
    assert_eq!(tail.last().unwrap().data["reason"], "stopped");
}

// ─── who may ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn both_roles_may_read_an_unknown_token_may_not_and_the_host_is_checked() {
    let mut f = Fleet::new("h5gw-roles");
    f.add("alpha", None, None);
    f.push_n("alpha", 1);
    f.hold("alpha");
    let gw = f.serve().await;
    let url = format!("{}/api/i/alpha/v1/events", gw.base);
    // owner (no token)
    let mut o = Stream::open(&gw, "/api/i/alpha/v1/events", |r| r).await;
    assert_eq!(o.records(1).await, [1]);
    // read-only, by bearer and by ?token= (an event stream cannot set headers)
    let mut a = Stream::open(&gw, "/api/i/alpha/v1/events", |r| r.bearer_auth(READ_TOKEN)).await;
    assert_eq!(a.records(1).await, [1]);
    let q = format!("/api/i/alpha/v1/events?token={READ_TOKEN}&after=0");
    let mut b = Stream::open(&gw, &q, |r| r).await;
    assert_eq!(b.records(1).await, [1]);
    // an unknown token, wherever it is
    let (s, _) = answer(client().get(&url).bearer_auth("nope-not-a-token")).await;
    assert_eq!(s, 401);
    let (s, _) = answer(client().get(format!("{url}?token=nope"))).await;
    assert_eq!(s, 401);
    // the owner role is Host-checked; the read-only role is not
    let (s, e) = answer(client().get(&url).header("host", "evil.example")).await;
    assert_eq!((s, e["error"].as_str()), (403, Some("bad_host")));
    let r = client()
        .get(&url)
        .header("host", "evil.example")
        .bearer_auth(READ_TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    // the door only reads: a write by the owner is not allowed here, one by the
    // read-only role is refused as a write, and a write needs a same Origin
    let (s, e) = answer(client().post(&url)).await;
    assert_eq!((s, e["error"].as_str()), (405, Some("method_not_allowed")));
    let (s, e) = answer(client().post(&url).bearer_auth(READ_TOKEN)).await;
    assert_eq!((s, e["error"].as_str()), (403, Some("read_only")));
    let (s, e) = answer(client().post(&url).header("origin", "http://evil.example")).await;
    assert_eq!((s, e["error"].as_str()), (403, Some("bad_origin")));
}

// ─── redaction on every frame ────────────────────────────────────────────────

fn config_naming(dir: &Path, token_env: &str, key_env: &str) -> PathBuf {
    let p = dir.join("host.yaml");
    std::fs::write(
        &p,
        format!(
            "name: x\nstate: /x\nengine: {{kind: agent, api_key_env: {key_env}}}\n\
             acp:\n  http: 127.0.0.1:0\n  tokens:\n    owner: {token_env}\n"
        ),
    )
    .unwrap();
    p
}

#[tokio::test]
async fn a_secret_in_a_record_line_never_appears_in_a_frame() {
    unsafe { std::env::set_var("H5GW_KEY_ENV", PLAIN) };
    let mut f = Fleet::new("h5gw-red-record");
    let cfg = config_naming(f.root.path(), "H5GW_NO_TOKEN_ENV", "H5GW_KEY_ENV");
    f.add("alpha", Some(&cfg), None);
    f.push(
        "alpha",
        "note.written",
        json!({"text": format!("a {PLAIN} and a {SHAPED}")}),
    );
    f.push(
        "alpha",
        "turn.ended",
        json!({"turn": 1, "final_text": format!("done {SHAPED}"), "tool": {"out": PLAIN}}),
    );
    f.push(
        "alpha",
        "halted",
        json!({"path": f.state("alpha").display().to_string()}),
    );
    let gw = f.serve().await;
    for who in [None, Some(READ_TOKEN)] {
        let mut s = Stream::open(&gw, "/api/i/alpha/v1/events", |r| match who {
            Some(t) => r.bearer_auth(t),
            None => r,
        })
        .await;
        let frames = s.rest().await;
        let wire: String = frames.iter().map(|x| x.data.to_string()).collect();
        assert!(frames.iter().any(|x| x.event == "record"));
        assert!(!wire.contains(PLAIN), "a plain secret reached a frame");
        assert!(!wire.contains(SHAPED), "a shaped secret reached a frame");
        assert!(wire.contains("[redacted]"));
    }
    // the record on disk is verbatim
    let disk =
        std::fs::read_to_string(f.state("alpha").join("record/seg-00000001.ndjson")).unwrap();
    assert!(disk.contains(PLAIN) && disk.contains(SHAPED));
}

// ─── partial frames from the host's own door ─────────────────────────────────

/// A fake host listener speaking the shape of `GET /v1/events?partials=only`:
/// it checks the bearer token and the path, sends the frames it is given, and
/// keeps the connection open until the test drops its end or the gateway
/// closes.
struct FakeHost {
    addr: String,
    send: tokio::sync::mpsc::UnboundedSender<String>,
    /// Resolves when the gateway closed its connection to the fake.
    closed: tokio::sync::oneshot::Receiver<()>,
    seen: tokio::sync::oneshot::Receiver<String>,
}

async fn fake_host() -> FakeHost {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let (send, mut recv) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (closed_tx, closed) = tokio::sync::oneshot::channel();
    let (seen_tx, seen) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut sock, _) = l.accept().await.unwrap();
        let mut head = Vec::new();
        let mut b = [0u8; 1024];
        while !head.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = sock.read(&mut b).await.unwrap();
            if n == 0 {
                return;
            }
            head.extend_from_slice(&b[..n]);
        }
        let _ = seen_tx.send(String::from_utf8_lossy(&head).to_string());
        sock.write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\n\r\n",
        )
        .await
        .unwrap();
        loop {
            tokio::select! {
                f = recv.recv() => match f {
                    Some(f) => { if sock.write_all(f.as_bytes()).await.is_err() { break; } }
                    None => break,
                },
                n = sock.read(&mut b) => { if n.unwrap_or(0) == 0 { break; } }
            }
        }
        let _ = closed_tx.send(());
    });
    FakeHost {
        addr,
        send,
        closed,
        seen,
    }
}

fn delta(v: Value) -> String {
    format!("event: delta\ndata: {v}\n\n")
}

#[tokio::test]
async fn partial_frames_of_a_running_host_are_relayed_redacted_and_not_recorded() {
    unsafe {
        std::env::set_var("H5GW_KEY_ENV", PLAIN);
        std::env::set_var("H5GW_TOKEN_ENV", DOOR_TOKEN);
    }
    let fake = fake_host().await;
    let mut f = Fleet::new("h5gw-partial");
    let cfg = config_naming(f.root.path(), "H5GW_TOKEN_ENV", "H5GW_KEY_ENV");
    f.add("alpha", Some(&cfg), Some(&fake.addr));
    f.push_n("alpha", 2);
    f.hold("alpha");
    let gw = f.serve().await;
    let mut s = Stream::open(&gw, "/api/i/alpha/v1/events", |r| r).await;
    let open = s.next().await.unwrap();
    assert_eq!(open.data["partials"], true);
    assert_eq!(s.records(2).await, [1, 2]);
    // the gateway asked the host's door, with the host's own token
    let head = tokio::time::timeout(GUARD, fake.seen)
        .await
        .unwrap()
        .unwrap();
    assert!(
        head.starts_with("GET /v1/events?partials=only"),
        "{}",
        &head[..40.min(head.len())]
    );
    assert!(head.contains(&format!("Bearer {DOOR_TOKEN}")));
    // the host's frames: the key split between two pieces, then a tool
    let (a, b) = PLAIN.split_at(10);
    let before = std::fs::read(f.state("alpha").join("record/seg-00000001.ndjson")).unwrap();
    for v in [
        json!({"turn": 3, "call": 0, "kind": "block_start", "id": null}),
        json!({"turn": 3, "call": 0, "kind": "text", "text": format!("my key is {a}")}),
        json!({"turn": 3, "call": 0, "kind": "text", "text": format!("{b} and that is that. ")}),
        json!({"turn": 3, "call": 0, "kind": "block_end", "id": null}),
        json!({"turn": 3, "call": 0, "kind": "tool_run", "name": "shell", "input": {"cmd": format!("echo {SHAPED}")}}),
        json!({"turn": 3, "call": 0, "kind": "tool_done", "name": "shell", "ok": true, "output": format!("{PLAIN}\n")}),
        json!({"turn": 3, "call": 0, "kind": "turn_end"}),
    ] {
        fake.send.send(delta(v)).unwrap();
    }
    let mut text = String::new();
    let mut wire = String::new();
    loop {
        let fr = s.next().await.unwrap();
        assert_eq!(fr.event, "delta");
        assert!(fr.id.is_none());
        wire.push_str(&fr.data.to_string());
        if fr.data["kind"] == "text" {
            text.push_str(fr.data["text"].as_str().unwrap());
        }
        if fr.data["kind"] == "turn_end" {
            break;
        }
    }
    assert!(!wire.contains(PLAIN), "a plain secret reached a delta");
    assert!(!wire.contains(SHAPED), "a shaped secret reached a delta");
    assert!(
        text == "my key is [redacted] and that is that. ",
        "the relayed text is not what was expected"
    );
    assert_eq!(
        std::fs::read(f.state("alpha").join("record/seg-00000001.ndjson")).unwrap(),
        before,
        "a partial frame reached the record"
    );
    // a client that goes away frees the gateway's hold on the host's door
    drop(s);
    tokio::time::timeout(GUARD, fake.closed)
        .await
        .expect("the gateway kept the host's stream open for a gone client")
        .unwrap();
}

#[tokio::test]
async fn a_block_the_host_began_before_the_relay_joined_is_not_relayed_and_the_next_is() {
    unsafe {
        std::env::set_var("H5GW_KEY_ENV", PLAIN);
        std::env::set_var("H5GW_TOKEN_ENV", DOOR_TOKEN);
    }
    let fake = fake_host().await;
    let mut f = Fleet::new("h5gw-join");
    let cfg = config_naming(f.root.path(), "H5GW_TOKEN_ENV", "H5GW_KEY_ENV");
    f.add("alpha", Some(&cfg), Some(&fake.addr));
    f.push_n("alpha", 1);
    f.hold("alpha");
    let gw = f.serve().await;
    let mut s = Stream::open(&gw, "/api/i/alpha/v1/events", |r| r).await;
    assert_eq!(s.next().await.unwrap().data["partials"], true);
    assert_eq!(s.records(1).await, [1]);
    // the relay is in the middle of a block: the front of the key was never seen
    let (a, b) = PLAIN.split_at(12);
    for v in [
        json!({"turn": 4, "call": 0, "kind": "text", "text": format!("{b} and the rest of that block. ")}),
        json!({"turn": 4, "call": 0, "kind": "block_end", "id": null}),
        json!({"turn": 4, "call": 0, "kind": "behind", "skipped": 7}),
        json!({"turn": 4, "call": 1, "kind": "block_start", "id": null}),
        json!({"turn": 4, "call": 1, "kind": "text", "text": "A block seen from its start, with enough words to go out. "}),
        json!({"turn": 4, "call": 1, "kind": "block_end", "id": null}),
        json!({"turn": 4, "call": 1, "kind": "turn_end"}),
    ] {
        fake.send.send(delta(v)).unwrap();
    }
    let mut text = String::new();
    let mut wire = String::new();
    loop {
        let fr = s.next().await.unwrap();
        wire.push_str(&fr.data.to_string());
        if fr.data["kind"] == "text" {
            text.push_str(fr.data["text"].as_str().unwrap());
        }
        if fr.data["kind"] == "turn_end" {
            break;
        }
    }
    assert!(
        !wire.contains(b) && !wire.contains(a),
        "a piece of a secret reached a client"
    );
    assert!(
        !text.contains("rest of that block"),
        "a block joined in the middle was relayed"
    );
    assert!(text.contains("A block seen from its start"));
}

#[tokio::test]
async fn without_a_door_to_the_host_the_record_still_streams_and_says_no_partials() {
    let mut f = Fleet::new("h5gw-nopartial");
    // no address at all
    f.add("a", None, None);
    f.push_n("a", 1);
    f.hold("a");
    // an address nothing listens on
    let dead = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().to_string()
    };
    unsafe { std::env::set_var("H5GW_TOKEN_ENV", DOOR_TOKEN) };
    let cfg = config_naming(f.root.path(), "H5GW_TOKEN_ENV", "H5GW_KEY_ENV");
    f.add("b", Some(&cfg), Some(&dead));
    f.push_n("b", 1);
    f.hold("b");
    let gw = f.serve().await;
    for id in ["a", "b"] {
        let mut s = Stream::open(&gw, &format!("/api/i/{id}/v1/events"), |r| r).await;
        let open = s.next().await.unwrap();
        assert_eq!(open.data["partials"], false, "{id}");
        assert_eq!(s.records(1).await, [1]);
        f.push_n(id, 1);
        assert_eq!(s.records(1).await, [2]);
    }
    // a client that does not want partial frames never has the door asked
    let mut s = Stream::open(&gw, "/api/i/b/v1/events?partials=off", |r| r).await;
    assert_eq!(s.next().await.unwrap().data["partials"], false);
}

// ─── many clients, a stalled one, and the thread pool ────────────────────────

#[tokio::test]
async fn many_clients_and_a_stalled_one_do_not_block_each_other() {
    let mut f = Fleet::new("h5gw-many");
    f.add("alpha", None, None);
    f.push_n("alpha", 10);
    f.hold("alpha");
    let gw = f.serve().await;
    // a client that asks for the stream and never reads
    let mut stalled = std::net::TcpStream::connect(gw.addr).unwrap();
    write!(
        stalled,
        "GET /api/i/alpha/v1/events HTTP/1.1\r\nHost: localhost\r\n\r\n"
    )
    .unwrap();
    let mut readers = Vec::new();
    for _ in 0..25 {
        let mut s = Stream::open(&gw, "/api/i/alpha/v1/events", |r| r).await;
        readers.push(tokio::spawn(async move {
            let a = s.records(10).await;
            let b = s.records(10).await;
            (a, b)
        }));
    }
    // more than a socket buffer holds for the stalled one
    for i in 0..10 {
        f.push(
            "alpha",
            "note.written",
            json!({"text": "x".repeat(200_000), "i": i}),
        );
    }
    for r in readers {
        let (a, b) = tokio::time::timeout(GUARD, r).await.unwrap().unwrap();
        assert_eq!(a, (1..=10).collect::<Vec<_>>());
        assert_eq!(b, (11..=20).collect::<Vec<_>>());
    }
    drop(stalled);
}

#[test]
fn waiting_clients_hold_no_blocking_thread() {
    // Two blocking threads in the whole runtime. If each waiting stream held
    // one, the summary door (which reads on a blocking thread) could not answer.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(2)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut f = Fleet::new("h5gw-pool");
        f.add("alpha", None, None);
        f.push_n("alpha", 2);
        f.hold("alpha");
        let gw = f.serve().await;
        let mut streams = Vec::new();
        for _ in 0..30 {
            let mut s = Stream::open(&gw, "/api/i/alpha/v1/events", |r| r).await;
            assert_eq!(s.records(2).await, [1, 2]);
            streams.push(s);
        }
        let (s, _) = tokio::time::timeout(
            GUARD,
            answer(client().get(format!("{}/api/i/alpha/v1/summary", gw.base))),
        )
        .await
        .expect("thirty waiting streams starved the blocking pool");
        assert_eq!(s, 200);
        f.push_n("alpha", 1);
        for s in streams.iter_mut() {
            assert_eq!(s.records(1).await, [3]);
        }
    });
}
