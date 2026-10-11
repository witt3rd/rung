//! H5: the host's own `GET /v1/events` door on its HTTP listener, over real
//! HTTP. The listener is the host's (`acp_http`), the record is written by
//! hand into a state directory, and partial frames are published on the bus
//! the way a running turn does. A sentinel is never put in an assertion
//! message.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rung_agent_core::acp_http::{Serve, listen_bound};
use rung_host::acp::EventsDoor;
use rung_host::redact::Redactor;
use rung_host::statelock::StateLock;
use rung_host::stream::LiveBus;
use rung_testkit::TempDir;
use serde_json::{Value, json};

const TOKEN: &str = "door-token-h5-7731";
const PLAIN: &str = "plainsentinelh5door-0123456789";
const GUARD: Duration = Duration::from_secs(20);

struct Host {
    dir: TempDir,
    _lock: StateLock,
    bus: Arc<LiveBus>,
    base: String,
    next: u64,
}

impl Host {
    async fn start(tag: &str) -> Host {
        let dir = TempDir::new(tag);
        std::fs::create_dir_all(dir.path().join("record")).unwrap();
        let lock = StateLock::acquire(dir.path()).unwrap();
        let bus = LiveBus::new();
        let door = Arc::new(EventsDoor {
            state_dir: dir.path().to_path_buf(),
            bus: bus.clone(),
            red: Redactor::new().with_secret(PLAIN).with_secret(TOKEN),
            env_unset: vec!["SOME_UNSET_KEY_ENV".into()],
            tokens: [TOKEN.to_string()].into_iter().collect(),
        });
        let serve = Serve {
            name: "test".into(),
            authorize: Arc::new(|_| true),
            connect: Arc::new(|_, _| Box::pin(async {})),
            bind_bearer: false,
            routes: Some(door.routes()),
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = listen_bound("127.0.0.1:0".into(), serve, move |a| {
                let _ = tx.send(a);
            })
            .await;
        });
        let addr = rx.await.unwrap();
        Host {
            dir,
            _lock: lock,
            bus,
            base: format!("http://{addr}"),
            next: 1,
        }
    }

    fn push(&mut self, kind: &str, body: Value) {
        let mut m = body.as_object().cloned().unwrap_or_default();
        m.insert("seq".into(), self.next.into());
        m.insert("at".into(), (1000 + self.next).into());
        m.insert("kind".into(), kind.into());
        self.next += 1;
        let p: PathBuf = self.dir.path().join("record/seg-00000001.ndjson");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .unwrap();
        writeln!(f, "{}", Value::Object(m)).unwrap();
        self.bus.wake();
    }

    fn push_n(&mut self, n: u64) {
        for i in 0..n {
            self.push("note.written", json!({"text": format!("n{i}")}));
        }
    }

    fn state(&self) -> &Path {
        self.dir.path()
    }
}

fn http() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

/// One SSE frame as the client sees it.
#[derive(Debug)]
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
    async fn open(host: &Host, query: &str, headers: &[(&str, &str)]) -> Stream {
        let mut r = http()
            .get(format!("{}/v1/events{query}", host.base))
            .header("authorization", format!("Bearer {TOKEN}"));
        for (k, v) in headers {
            r = r.header(*k, *v);
        }
        let resp = tokio::time::timeout(GUARD, r.send())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resp.status(), 200);
        Stream {
            resp,
            buf: String::new(),
        }
    }

    /// The next frame that is not a keepalive comment; `None` at the end of
    /// the body.
    async fn next(&mut self) -> Option<Sse> {
        loop {
            if let Some(i) = self.buf.find("\n\n") {
                let block: String = self.buf.drain(..i + 2).collect();
                let mut event = String::new();
                let mut id = None;
                let mut data = String::new();
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
                    continue; // a comment
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
}

async fn status(host: &Host, path: &str, token: Option<&str>) -> (u16, Value) {
    let mut r = http().get(format!("{}{path}", host.base));
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    let resp = r.send().await.unwrap();
    let s = resp.status().as_u16();
    (s, resp.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn the_door_streams_the_record_from_a_cursor_then_live_over_http() {
    let mut h = Host::start("h5-door-live").await;
    h.push_n(4);
    let resp = http()
        .get(format!("{}/v1/events", h.base))
        .header("authorization", format!("Bearer {TOKEN}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers()["content-type"], "text/event-stream");
    assert!(
        resp.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("no-cache")
    );
    drop(resp);
    let mut s = Stream::open(&h, "", &[]).await;
    let open = s.next().await.unwrap();
    assert_eq!(open.event, "open");
    assert_eq!(open.data["contract"], 1);
    // the variables the host's config names and the host does not hold
    assert_eq!(
        open.data["redaction"]["env_unset"],
        json!(["SOME_UNSET_KEY_ENV"])
    );
    assert_eq!(s.records(4).await, [1, 2, 3, 4]);
    h.push_n(2);
    assert_eq!(s.records(2).await, [5, 6]);
    // from a cursor
    let mut c = Stream::open(&h, "?after=4", &[]).await;
    assert_eq!(c.records(2).await, [5, 6]);
}

#[tokio::test]
async fn last_event_id_beats_a_stale_after() {
    let mut h = Host::start("h5-door-lei").await;
    h.push_n(6);
    // an EventSource keeps the address it was given and sends Last-Event-ID
    let mut s = Stream::open(&h, "?after=1", &[("last-event-id", "4")]).await;
    assert_eq!(s.records(2).await, [5, 6]);
}

#[tokio::test]
async fn the_door_refuses_what_it_should_before_any_frame() {
    let mut h = Host::start("h5-door-refuse").await;
    h.push_n(2);
    assert_eq!(status(&h, "/v1/events", None).await.0, 401);
    assert_eq!(status(&h, "/v1/events", Some("not-a-token")).await.0, 401);
    let (s, body) = status(&h, "/v1/events?after=9", Some(TOKEN)).await;
    assert_eq!((s, body["error"].as_str()), (409, Some("cursor_ahead")));
    for q in ["?after=x", "?after=1&after=2", "?partials=maybe", "?what=1"] {
        let (s, body) = status(&h, &format!("/v1/events{q}"), Some(TOKEN)).await;
        assert_eq!(
            (s, body["error"].as_str()),
            (400, Some("bad_request")),
            "{q}"
        );
    }
    let resp = http()
        .post(format!("{}/v1/events", h.base))
        .header("authorization", format!("Bearer {TOKEN}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 405);
    // HEAD: the headers of a stream, and no body
    let head = http()
        .head(format!("{}/v1/events", h.base))
        .header("authorization", format!("Bearer {TOKEN}"))
        .send()
        .await
        .unwrap();
    assert_eq!(head.status(), 200);
    assert_eq!(head.headers()["content-type"], "text/event-stream");
    // another path is not this door
    assert_eq!(status(&h, "/v1/nothing", Some(TOKEN)).await.0, 404);
    // the token never comes back, even in an error that echoes the request
    let (_, body) = status(&h, &format!("/v1/events?what={TOKEN}"), Some(TOKEN)).await;
    assert!(!body.to_string().contains(TOKEN), "a token came back");
    h.push_n(0);
}

#[tokio::test]
async fn partial_frames_arrive_flagged_redacted_and_are_not_in_the_record() {
    let mut h = Host::start("h5-door-partial").await;
    h.push_n(1);
    let mut s = Stream::open(&h, "", &[]).await;
    assert_eq!(s.records(1).await, [1]);
    let before = std::fs::read(h.state().join("record/seg-00000001.ndjson")).unwrap();
    let t = h.bus.turn(1);
    t.block_start(0);
    t.text(0, &format!("the key is {PLAIN} so "));
    t.text(0, "keep it safe. ");
    t.block_end(0, None);
    t.tool_run(0, "ws_write", &json!({"text": PLAIN}));
    t.tool_done(0, "ws_write", true, &format!("wrote {PLAIN}"));
    t.end();
    let mut text = String::new();
    let mut kinds = Vec::new();
    loop {
        let f = s.next().await.unwrap();
        assert_eq!(f.event, "delta");
        assert!(f.id.is_none(), "a delta has no id");
        assert!(!f.data.to_string().contains(PLAIN), "a delta leaked");
        if f.data["kind"] == "text" {
            text.push_str(f.data["text"].as_str().unwrap());
        }
        let k = f.data["kind"].as_str().unwrap().to_string();
        kinds.push(k.clone());
        if k == "turn_end" {
            break;
        }
    }
    assert!(
        text == "the key is [redacted] so keep it safe. ",
        "the redacted text is not what was expected"
    );
    assert!(kinds.contains(&"tool_run".to_string()) && kinds.contains(&"tool_done".to_string()));
    assert_eq!(
        std::fs::read(h.state().join("record/seg-00000001.ndjson")).unwrap(),
        before,
        "a partial frame wrote the record"
    );
    // off: no deltas
    let mut off = Stream::open(&h, "?partials=off", &[]).await;
    assert_eq!(off.records(1).await, [1]);
}

#[tokio::test]
async fn a_disconnected_client_frees_its_subscription() {
    let mut h = Host::start("h5-door-free").await;
    h.push_n(1);
    let mut s = Stream::open(&h, "", &[]).await;
    assert_eq!(s.records(1).await, [1]);
    assert_eq!(h.bus.subscribers(), 1);
    drop(s);
    // the host notices the client is gone the next time it would write, or at
    // the latest at its keepalive: a bounded wait for the count, not a sleep
    tokio::time::timeout(GUARD, async {
        while h.bus.subscribers() != 0 {
            h.bus.turn(9).text(0, "poke ");
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("a gone client kept its subscription");
}

#[tokio::test]
async fn many_clients_and_a_stalled_one_do_not_block_each_other() {
    let mut h = Host::start("h5-door-many").await;
    h.push_n(10);
    // a client that asks and never reads
    let stalled = std::net::TcpStream::connect(h.base.trim_start_matches("http://")).unwrap();
    let mut w = &stalled;
    write!(
        w,
        "GET /v1/events HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
    )
    .unwrap();
    let mut readers = Vec::new();
    for _ in 0..25 {
        let mut s = Stream::open(&h, "", &[]).await;
        readers.push(tokio::spawn(async move {
            let a = s.records(10).await;
            let b = s.records(10).await;
            (a, b)
        }));
    }
    // a lot of lines, more than a socket buffer holds for the stalled one
    for i in 0..10 {
        h.push("note.written", json!({"text": "x".repeat(200_000), "i": i}));
    }
    for r in readers {
        let (a, b) = tokio::time::timeout(GUARD, r).await.unwrap().unwrap();
        assert_eq!(a, (1..=10).collect::<Vec<_>>());
        assert_eq!(b, (11..=20).collect::<Vec<_>>());
    }
    drop(stalled);
}
