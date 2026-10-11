//! H3: the read doors over the native instances' own state directories.
//!
//! The gateway reads each registered instance's record (and, for its state
//! word, the registry entry and the state lock) and answers three reads: the
//! list (`GET /api/instances`), the fold (`GET /api/i/{id}/v1/summary`) and the
//! recorded events, paged (`GET /api/i/{id}/v1/record`). Nothing here sleeps on
//! a clock and nothing starts a host: the state directories are written by
//! hand, and a "running" instance is one whose state lock a test holds.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rung_gateway::Settings;
use rung_host::statelock::StateLock;
use rung_testkit::TempDir;
use serde_json::{Value, json};

const READ_TOKEN: &str = "view-token-canary-h3-5521";
/// The owner key the gateway holds for a configured instance.
const OWNER_KEY: &str = "KEY-canary-h3-owner-key-0042";
/// A sentinel with a known key shape: found by the redactor with no help.
const SHAPED: &str = "sk-SENTINELh3shaped0123456789abcdef";
/// A sentinel with no shape: only found because the gateway knows its value.
const PLAIN: &str = "plainsentinelh3-0123456789";

const MIN: i64 = 60_000;
const T0: i64 = 1_791_208_186_000;

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

// ─── fixtures ────────────────────────────────────────────────────────────────

struct Fleet {
    root: TempDir,
    locks: Vec<StateLock>,
}

fn line(seq: u64, at: i64, kind: &str, body: Value) -> Value {
    let mut m = body.as_object().cloned().unwrap_or_default();
    m.insert("seq".into(), seq.into());
    m.insert("at".into(), at.into());
    m.insert("kind".into(), kind.into());
    Value::Object(m)
}

/// Lines with consecutive numbers from 1, one every `step` ms.
fn numbered(kinds: Vec<(&str, Value)>) -> Vec<Value> {
    kinds
        .into_iter()
        .enumerate()
        .map(|(i, (k, b))| line(i as u64 + 1, T0 + i as i64 * 1000, k, b))
        .collect()
}

fn text(lines: &[Value]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
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

    /// A registry entry for `id`, its state directory made, no record yet.
    fn add(&self, id: &str, config: Option<&Path>) -> PathBuf {
        let state = self.state(id);
        std::fs::create_dir_all(&state).unwrap();
        let entry = json!({
            "schema": 1, "id": id, "name": id.to_uppercase(),
            "state_dir": state, "config": config,
            "workspace": self.root.path().join(id).join("workspace"),
            "address": null, "pid": 4242, "started": T0, "version": "0.2.3",
        });
        std::fs::write(
            self.registry().join(format!("{id}.json")),
            serde_json::to_string_pretty(&entry).unwrap(),
        )
        .unwrap();
        state
    }

    /// One segment holding `lines`.
    fn record(&self, id: &str, lines: &[Value]) {
        let dir = self.state(id).join("record");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("seg-00000001.ndjson"), text(lines)).unwrap();
    }

    /// The host is alive: its state lock is held, by the test.
    fn hold(&mut self, id: &str) {
        self.locks
            .push(StateLock::acquire(&self.state(id)).expect("lock"));
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
        self.serve_with(self.settings()).await
    }

    async fn serve_with(&self, s: Settings) -> Gw {
        let running = rung_gateway::start(s).await.unwrap();
        Gw {
            base: format!("http://{}", running.addr),
            _running: running,
        }
    }
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

impl Gw {
    async fn get(&self, path: &str) -> Answer {
        self.send(client().get(format!("{}{path}", self.base)))
            .await
    }

    async fn get_as_reader(&self, path: &str) -> Answer {
        self.send(
            client()
                .get(format!("{}{path}", self.base))
                .bearer_auth(READ_TOKEN),
        )
        .await
    }

    async fn send(&self, rb: reqwest::RequestBuilder) -> Answer {
        let r = rb.send().await.unwrap();
        Answer {
            status: r.status().as_u16(),
            body: r.text().await.unwrap(),
        }
    }
}

/// A record that tells a small story: a start, a project, an owner message
/// waiting, a commitment, a turn in flight.
fn story() -> Vec<Value> {
    numbered(vec![
        (
            "host.start",
            json!({"pid": 4242, "desk": "jev", "desk_mode": "shadow", "memory": "baseline",
                   "config": {"engine": "agent", "ladder": ["model-a", "model-b"],
                              "governor": {"quota": {"rpd": 400, "rpm": 20}, "spend_cap_usd_day": null}}}),
        ),
        (
            "project.added",
            json!({"id": "notes", "title": "Tidy the notes", "why": "seed", "status": "seed", "turn": 0}),
        ),
        ("boundary", json!({"n": 1, "mode": "free", "pending": 0})),
        (
            "kernel.commit",
            json!({"turn": 1, "project": "notes", "title": "Tidy the notes index", "done_when": "the index is sorted"}),
        ),
        (
            "stimulus.accepted",
            json!({"item": {"id": "m-1", "kind": "peer", "role": "owner", "channel": "owner",
                            "at": T0 + 4000, "firm": false, "text": "please look at the build"}}),
        ),
        (
            "stimulus.accepted",
            json!({"item": {"id": "m-2", "kind": "peer", "role": "peer", "channel": "peer:b",
                            "at": T0 + 5000, "firm": false, "text": "second in line"}}),
        ),
        (
            "boundary",
            json!({"n": 2, "mode": "committed:notes", "pending": 2}),
        ),
        (
            "turn.started",
            json!({"turn": 2, "turn_kind": "committed", "mode": "committed:notes", "project": "notes",
                   "model": "model-a", "rung": 0, "epoch": 1}),
        ),
        (
            "llm.call",
            json!({"turn": 2, "call": 1, "turn_kind": "committed", "model_served": "model-a",
                   "prompt_tokens": 100, "completion_tokens": 20, "cost_usd": 0.0125}),
        ),
    ])
}

fn ids(page: &Value) -> Vec<u64> {
    page["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["seq"].as_u64().unwrap())
        .collect()
}

// ─── the list ────────────────────────────────────────────────────────────────

/// The list names each native instance with the word the registry and the
/// lock give it, the command that starts it, and the time of its last event.
#[tokio::test]
async fn the_list_names_each_instance_its_word_its_start_command_and_last_event() {
    let mut f = Fleet::new("h3-list");
    let cfg = f.root.path().join("beta.yaml");
    f.add("alpha", None);
    f.record("alpha", &story());
    f.hold("alpha");
    f.add("beta", Some(&cfg));
    f.record(
        "beta",
        &numbered(vec![
            ("host.start", json!({"config": {}})),
            ("halted", json!({"why": "stop"})),
        ]),
    );
    f.add("gamma", None);
    f.record(
        "gamma",
        &numbered(vec![("host.start", json!({"config": {}}))]),
    );
    let gw = f.serve().await;

    let a = gw.get("/api/instances").await;
    assert_eq!(a.status, 200, "{}", a.body);
    let rows = a.json()["instances"].as_array().unwrap().clone();
    let by: BTreeMap<String, Value> = rows
        .iter()
        .map(|r| (r["id"].as_str().unwrap().to_string(), r.clone()))
        .collect();
    assert_eq!(by.keys().collect::<Vec<_>>(), ["alpha", "beta", "gamma"]);

    assert_eq!(by["alpha"]["word"], "running");
    assert_eq!(by["beta"]["word"], "stopped");
    assert_eq!(by["gamma"]["word"], "down");
    assert_eq!(by["alpha"]["name"], "ALPHA");
    assert_eq!(by["alpha"]["source"], "registry");
    assert_eq!(by["alpha"]["reachable"], true);
    // the last event's time and kind
    assert_eq!(by["alpha"]["last_at"], T0 + 8000);
    assert_eq!(by["alpha"]["last"]["seq"], 9);
    assert_eq!(by["beta"]["last"]["kind"], "halted");
    // the start command: for a config-file host, the real command; none otherwise
    let cmd = by["beta"]["start_command"].as_str().unwrap();
    assert!(
        cmd.contains("rung-host run --config") && cmd.contains("beta.yaml"),
        "{cmd}"
    );
    assert!(by["alpha"]["start_command"].is_null());
    // each row carries its fold's summary
    assert_eq!(by["alpha"]["summary"]["state"], "Working");
    assert_eq!(by["beta"]["summary"]["state"], "Stopped");
    assert_eq!(by["gamma"]["summary"]["state"], "Down");
}

/// An instance nobody can read still has a row; so does a registry entry
/// whose file is not an entry.
#[tokio::test]
async fn an_unreadable_entry_is_a_row_with_its_problem() {
    let f = Fleet::new("h3-list-bad");
    f.add("ok", None);
    std::fs::write(f.registry().join("broken.json"), "{ not json").unwrap();
    let gw = f.serve().await;
    let rows = gw.get("/api/instances").await.json()["instances"].clone();
    let broken = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "broken")
        .expect("listed");
    assert_eq!(broken["word"], "unreadable");
    assert!(broken["problem"].as_str().unwrap().contains("broken.json"));
    // and its doors say so, rather than panic
    let s = gw.get("/api/i/broken/v1/summary").await;
    assert_eq!(s.status, 500, "{}", s.body);
    assert_eq!(s.json()["error"], "unreadable");
}

/// No registry folder, or an absent one, lists nothing.
#[tokio::test]
async fn no_registry_folder_is_an_empty_list() {
    let f = Fleet::new("h3-list-none");
    let mut s = f.settings();
    s.registry_dir = Some(f.root.path().join("nowhere"));
    let gw = f.serve_with(s).await;
    let a = gw.get("/api/instances").await;
    assert_eq!(a.status, 200);
    assert_eq!(a.json()["instances"], json!([]));
    let a = gw.get("/api/i/alpha/v1/summary").await;
    assert_eq!(a.status, 404);
    assert_eq!(a.json()["error"], "no_such_instance");
}

// ─── the fold ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_fold_says_what_the_instance_is_doing_and_waiting_for() {
    let mut f = Fleet::new("h3-fold");
    let cfg = f.root.path().join("alpha.yaml");
    std::fs::write(
        &cfg,
        "name: Alpha\nstate: /x\nidentity: be useful\nengine: {kind: agent, api_key_env: SOME_ENV}\n\
         desk: {mode: shadow, cap_usd_day: 0.5, cap_usd_ask: 0.002}\n",
    )
    .unwrap();
    f.add("alpha", Some(&cfg));
    f.record("alpha", &story());
    f.hold("alpha");
    let gw = f.serve().await;

    let a = gw.get("/api/i/alpha/v1/summary").await;
    assert_eq!(a.status, 200, "{}", a.body);
    let s = a.json();
    assert_eq!(s["contract"], 1);
    assert_eq!(s["id"], "alpha");
    assert_eq!(s["word"], "running");
    assert_eq!(s["state"], "Working");
    assert!(
        s["doing"]
            .as_str()
            .unwrap()
            .contains("Tidy the notes index")
    );
    assert_eq!(s["needs_you"], false);
    assert_eq!(s["turn"], 2);
    assert_eq!(s["last_seq"], 9);
    assert_eq!(s["last_at"], T0 + 8000);
    assert_eq!(s["requests"], 1);
    assert_eq!(s["quota"], 400);
    assert_eq!(s["spend_usd_day"], 0.0125);
    // the turn running now, and the boundary it came from
    assert_eq!(s["turn_in_progress"]["turn"], 2);
    assert_eq!(s["turn_in_progress"]["model"], "model-a");
    assert_eq!(s["boundary"]["n"], 2);
    assert_eq!(s["boundary"]["pending"], 2);
    assert_eq!(s["waiting_for"]["kind"], "turn");
    // the queue head: the oldest waiting item, whole
    assert_eq!(s["queue"]["waiting"], 2);
    assert_eq!(s["queue"]["head"]["id"], "m-1");
    assert_eq!(s["queue"]["head"]["text"], "please look at the build");
    // the desk's limits, and the config as read
    assert_eq!(s["desk"]["backend"], "jev");
    assert_eq!(s["desk"]["limits"]["cap_usd_day"], 0.5);
    assert_eq!(s["desk"]["limits"]["max_interrupts_per_hour"], 6);
    assert_eq!(s["config"]["file"]["content"]["identity"], "be useful");
    assert_eq!(s["config"]["started_with"]["engine"], "agent");
    assert!(s["fold_hash"].as_str().unwrap().len() >= 16);
    assert!(s["damaged"].is_null());
    assert!(s["problem"].is_null());
}

#[tokio::test]
async fn the_state_word_follows_the_record_and_the_lock() {
    let mut f = Fleet::new("h3-words");
    let mut put = |id: &str, tail: Vec<(&str, Value)>, held: bool| {
        f.add(id, None);
        let mut v = vec![("host.start", json!({"config": {}}))];
        v.extend(tail);
        f.record(id, &numbered(v));
        if held {
            f.hold(id);
        }
    };
    put(
        "free",
        vec![("boundary", json!({"n": 1, "mode": "free", "pending": 0}))],
        true,
    );
    put(
        "waiting",
        vec![(
            "degraded",
            json!({"class": "quota", "until": T0 + 90 * MIN, "why": "the day's quota is used"}),
        )],
        true,
    );
    put(
        "stuck",
        vec![(
            "degraded",
            json!({"class": "blocked", "until": T0 + 90 * MIN, "why": "the key was refused"}),
        )],
        true,
    );
    put(
        "answering",
        vec![
            (
                "stimulus.accepted",
                json!({"item": {"id": "o1", "kind": "peer", "role": "owner", "channel": "owner",
                                "at": T0, "firm": false, "text": "hi"}}),
            ),
            (
                "stimulus.admitted",
                json!({"turn": 1, "boundary": 1, "ids": ["o1"], "digests": []}),
            ),
            (
                "turn.started",
                json!({"turn": 1, "turn_kind": "responding", "model": "m"}),
            ),
        ],
        true,
    );
    put(
        "down",
        vec![("boundary", json!({"n": 1, "mode": "free", "pending": 0}))],
        false,
    );
    let gw = f.serve().await;
    let state = |id: &'static str| {
        let gw = &gw;
        async move {
            let s = gw.get(&format!("/api/i/{id}/v1/summary")).await.json();
            (
                s["state"].as_str().unwrap().to_string(),
                s["needs_you"].clone(),
                s["waiting_for"].clone(),
            )
        }
    };
    assert_eq!(state("free").await.0, "Free time");
    let (w, _, wf) = state("waiting").await;
    assert_eq!(w, "Waiting");
    assert_eq!(wf["kind"], "degraded");
    assert_eq!(wf["class"], "quota");
    assert_eq!(wf["until"], T0 + 90 * MIN);
    let (s, needs, _) = state("stuck").await;
    assert_eq!((s.as_str(), &needs), ("Stuck", &json!(true)));
    assert_eq!(state("answering").await.0, "Answering");
    let (d, needs, _) = state("down").await;
    assert_eq!((d.as_str(), &needs), ("Down", &json!(true)));
}

/// The fold of a recorded run equals what the live host computed at each
/// turn end: every `turn.ended` line carries the hash of the state replayed up
/// to it, and the door's lines replay to the same hashes. The summary's own
/// hash is the replay of the whole record.
#[tokio::test]
async fn the_fold_of_a_recorded_run_equals_the_live_projection_at_every_turn_end() {
    let run = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rung-host/live/runs/2026-10-05-hostkey-fixed/state/record");
    let f = Fleet::new("h3-fixture");
    f.add("run", None);
    std::fs::create_dir_all(f.state("run").join("record")).unwrap();
    for e in std::fs::read_dir(&run).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), f.state("run").join("record").join(e.file_name())).unwrap();
    }
    let gw = f.serve().await;

    let page = gw
        .get("/api/i/run/v1/record?limit=99999999999999999999999")
        .await;
    assert_eq!(page.status, 200);
    let page = page.json();
    let lines: Vec<rung_host::record::Line> = page["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| rung_host::record::Line::parse(&l.to_string()).expect("a record line"))
        .collect();
    assert_eq!(page["total"], lines.len());
    assert!(lines.len() > 2000, "{} lines", lines.len());
    assert!(page["next"].is_null());
    let hashes = rung_host::state::State::replay_hashes(&lines);
    let ended: Vec<&rung_host::record::Line> =
        lines.iter().filter(|l| l.kind == "turn.ended").collect();
    assert!(ended.len() > 10, "{} turns", ended.len());
    for l in &ended {
        assert_eq!(
            l.get("projection").as_str().unwrap(),
            hashes[&l.seq],
            "projection at turn.ended #{}",
            l.seq
        );
    }
    let whole = rung_host::state::State::replay(&lines).hash();
    let s = gw.get("/api/i/run/v1/summary").await.json();
    assert_eq!(s["fold_hash"], whole);
    assert_eq!(s["last_seq"], lines.last().unwrap().seq);
}

// ─── the paged events ────────────────────────────────────────────────────────

fn many(n: u64) -> Vec<Value> {
    (1..=n)
        .map(|i| {
            line(
                i,
                T0 + i as i64,
                "note.written",
                json!({"turn": i, "text": format!("note {i}")}),
            )
        })
        .collect()
}

#[tokio::test]
async fn the_record_is_paged_by_the_caller_with_exact_totals() {
    let f = Fleet::new("h3-paging");
    f.add("alpha", None);
    f.record("alpha", &many(25));
    let gw = f.serve().await;
    let rec = |q: &'static str| {
        let gw = &gw;
        async move { gw.get(&format!("/api/i/alpha/v1/record{q}")).await }
    };

    // page 1: the caller's size, the exact total, the next cursor
    let a = rec("?limit=10").await;
    assert_eq!(a.status, 200, "{}", a.body);
    let p = a.json();
    assert_eq!(ids(&p), (1..=10).collect::<Vec<_>>());
    assert_eq!(p["offset"], 0);
    assert_eq!(p["limit"], 10);
    assert_eq!(p["total"], 25);
    assert_eq!(p["next"], 10);
    assert_eq!(p["cursor"], 10);
    assert!(p["damaged"].is_null());
    // page 2 by offset, and by the cursor: the same lines
    let by_offset = rec("?limit=10&offset=10").await.json();
    let by_cursor = rec("?limit=10&after=10").await.json();
    assert_eq!(ids(&by_offset), (11..=20).collect::<Vec<_>>());
    assert_eq!(by_offset["lines"], by_cursor["lines"]);
    assert_eq!(by_offset["next"], 20);
    // the last page: short, no next, a cursor that still continues
    let last = rec("?limit=10&after=20").await.json();
    assert_eq!(ids(&last), (21..=25).collect::<Vec<_>>());
    assert!(last["next"].is_null());
    assert_eq!(last["cursor"], 25);
    assert_eq!(last["total"], 25);
    // a page exactly the rest has no next either
    assert!(rec("?limit=5&offset=20").await.json()["next"].is_null());
    // the default page is every line a small record has
    assert_eq!(ids(&rec("").await.json()).len(), 25);
    // a cursor past the end is an empty last page
    for q in ["?after=26", "?offset=26", "?after=1000000"] {
        let p = rec(q).await;
        assert_eq!(p.status, 200, "{q}: {}", p.body);
        let p = p.json();
        assert_eq!(p["lines"], json!([]), "{q}");
        assert!(p["next"].is_null(), "{q}");
        assert_eq!(p["total"], 25, "{q}");
    }
    // a huge page size is every line, uncapped, however large the number
    for q in [
        "?limit=18446744073709551615",
        "?limit=99999999999999999999999999",
        "?limit=1000000",
    ] {
        let p = rec(q).await.json();
        assert_eq!(ids(&p).len(), 25, "{q}");
        assert!(p["next"].is_null(), "{q}");
    }
    // newest first: the offset counts from the newest line
    let d = rec("?order=desc&limit=10").await.json();
    assert_eq!(ids(&d), (16..=25).rev().collect::<Vec<_>>());
    assert_eq!(d["next"], 10);
    let d2 = rec("?order=desc&limit=10&offset=20").await.json();
    assert_eq!(ids(&d2), (1..=5).rev().collect::<Vec<_>>());
    assert!(d2["next"].is_null());
    // walking every page, oldest first, gives the whole record once, in order
    let mut seen = Vec::new();
    let mut after = 0u64;
    loop {
        let p = gw
            .get(&format!("/api/i/alpha/v1/record?limit=7&after={after}"))
            .await
            .json();
        seen.extend(ids(&p));
        if p["next"].is_null() {
            break;
        }
        after = p["cursor"].as_u64().unwrap();
    }
    assert_eq!(seen, (1..=25).collect::<Vec<_>>());
}

#[tokio::test]
async fn a_bad_query_is_a_clear_400() {
    let f = Fleet::new("h3-badq");
    f.add("alpha", None);
    f.record("alpha", &many(3));
    let gw = f.serve().await;
    for q in [
        "?limit=0",
        "?limit=-1",
        "?limit=ten",
        "?offset=x",
        "?after=x",
        "?order=sideways",
        "?offset=1&after=1",
        "?order=desc&after=1",
        "?colour=blue",
        "?limit=1&limit=2",
    ] {
        let a = gw.get(&format!("/api/i/alpha/v1/record{q}")).await;
        assert_eq!(a.status, 400, "{q}: {}", a.body);
        assert_eq!(a.json()["error"], "bad_request", "{q}");
    }
}

#[tokio::test]
async fn an_empty_instance_is_an_empty_fold_and_an_empty_page() {
    let f = Fleet::new("h3-empty");
    f.add("none", None); // registered, never wrote a record
    f.add("blank", None);
    std::fs::create_dir_all(f.state("blank").join("record")).unwrap();
    let gw = f.serve().await;
    for id in ["none", "blank"] {
        let p = gw.get(&format!("/api/i/{id}/v1/record")).await;
        assert_eq!(p.status, 200, "{id}: {}", p.body);
        let p = p.json();
        assert_eq!(p["lines"], json!([]));
        assert_eq!(p["total"], 0);
        assert!(p["next"].is_null());
        assert_eq!(p["cursor"], 0);
        let s = gw.get(&format!("/api/i/{id}/v1/summary")).await;
        assert_eq!(s.status, 200, "{id}: {}", s.body);
        let s = s.json();
        assert_eq!(s["last_seq"], 0);
        assert!(s["turn"].is_null());
        assert!(s["queue"]["head"].is_null());
        assert_eq!(s["queue"]["waiting"], 0);
    }
}

/// A stopped instance reads the same as a running one: the record door's
/// pages are identical, and the fold differs only in what the lock says.
#[tokio::test]
async fn a_stopped_instance_reads_the_same_as_a_running_one() {
    let mut f = Fleet::new("h3-same");
    for id in ["live", "dead"] {
        f.add(id, None);
        f.record(id, &story());
    }
    f.hold("live");
    let gw = f.serve().await;
    let a = gw.get("/api/i/live/v1/record?limit=100").await.json();
    let b = gw.get("/api/i/dead/v1/record?limit=100").await.json();
    assert_eq!(a, b);
    let sa = gw.get("/api/i/live/v1/summary").await.json();
    let sb = gw.get("/api/i/dead/v1/summary").await.json();
    assert_eq!(sa["word"], "running");
    assert_eq!(sb["word"], "down");
    for k in [
        "turn",
        "last_seq",
        "last_at",
        "requests",
        "queue",
        "boundary",
        "desk",
        "fold_hash",
    ] {
        assert_eq!(sa[k], sb[k], "{k}");
    }
}

// ─── damage ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_torn_last_line_is_not_damage() {
    let f = Fleet::new("h3-torn");
    f.add("alpha", None);
    let mut t = text(&many(4));
    t.push_str("{\"at\":1,\"kind\":\"note.wri"); // a write in flight
    let dir = f.state("alpha").join("record");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("seg-00000001.ndjson"), t).unwrap();
    let gw = f.serve().await;
    let p = gw.get("/api/i/alpha/v1/record").await.json();
    assert_eq!(p["total"], 4);
    assert!(p["damaged"].is_null());
    assert_eq!(
        gw.get("/api/i/alpha/v1/summary").await.json()["last_seq"],
        4
    );
}

#[tokio::test]
async fn a_corrupt_record_reads_as_far_as_it_is_intact_and_says_where_it_is_not() {
    let f = Fleet::new("h3-corrupt");
    let cases: Vec<(&str, Vec<u8>, u64)> = vec![
        (
            "garbage in the middle",
            format!(
                "{}garbage not json\n{}",
                text(&many(3)),
                text(&many(6)[4..])
            )
            .into_bytes(),
            3,
        ),
        (
            "a gap in the numbers",
            format!("{}{}", text(&many(3)), text(&many(6)[4..])).into_bytes(),
            3,
        ),
        (
            "not a record line",
            format!("{}[1,2,3]\n", text(&many(2))).into_bytes(),
            2,
        ),
        (
            "bad bytes",
            [text(&many(1)).into_bytes(), vec![0xff, 0xfe, b'\n']].concat(),
            1,
        ),
    ];
    for (i, (_, content, _)) in cases.iter().enumerate() {
        let id = format!("c{i}");
        f.add(&id, None);
        let dir = f.state(&id).join("record");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("seg-00000001.ndjson"), content).unwrap();
    }
    let gw = f.serve().await;
    for (i, (name, _, intact)) in cases.iter().enumerate() {
        let p = gw.get(&format!("/api/i/c{i}/v1/record")).await;
        assert_eq!(p.status, 200, "{name}: {}", p.body);
        let p = p.json();
        assert_eq!(p["total"], *intact, "{name}");
        assert_eq!(ids(&p), (1..=*intact).collect::<Vec<_>>(), "{name}");
        assert!(p["next"].is_null(), "{name}");
        let d = &p["damaged"];
        assert_eq!(d["after_seq"], *intact, "{name}: {d}");
        assert!(
            d["segment"].as_str().unwrap().ends_with(".ndjson"),
            "{name}"
        );
        assert_eq!(d["line"], *intact + 1, "{name}");
        assert!(!d["problem"].as_str().unwrap().is_empty(), "{name}");
        // the fold is over the intact part and says it is not the whole
        let s = gw.get(&format!("/api/i/c{i}/v1/summary")).await;
        assert_eq!(s.status, 200, "{name}: {}", s.body);
        let s = s.json();
        assert_eq!(s["last_seq"], *intact, "{name}");
        assert_eq!(s["damaged"]["line"], *intact + 1, "{name}");
        assert!(s["problem"].as_str().unwrap().contains("record"), "{name}");
    }
}

#[tokio::test]
async fn a_dropped_state_directory_is_a_clear_error_never_a_panic() {
    let f = Fleet::new("h3-dropped");
    f.add("gone", None);
    std::fs::remove_dir_all(f.state("gone")).unwrap();
    let gw = f.serve().await;
    for door in ["summary", "record"] {
        let a = gw.get(&format!("/api/i/gone/v1/{door}")).await;
        assert_eq!(a.status, 404, "{door}: {}", a.body);
        let j = a.json();
        assert_eq!(j["error"], "state_dir_missing");
        assert!(j["message"].as_str().unwrap().contains("state"));
    }
    // the list still has it, down, with the problem
    let rows = gw.get("/api/instances").await.json()["instances"].clone();
    let row = &rows[0];
    assert_eq!(row["id"], "gone");
    assert_eq!(row["word"], "down");
    assert!(row["summary"].is_null());
    assert!(row["problem"].as_str().is_some());
    // a record that is a file where a directory should be: an error, not a panic
    f.add("odd", None);
    std::fs::write(f.state("odd").join("record"), "x").unwrap();
    let a = gw.get("/api/i/odd/v1/record").await;
    assert_eq!(
        (a.status, a.json()["error"].clone()),
        (500, json!("unreadable"))
    );
}

// ─── the doors' shape, roles and the gateway's guards ───────────────────────

#[tokio::test]
async fn an_unknown_instance_and_an_unserved_door_are_404() {
    let f = Fleet::new("h3-404");
    f.add("alpha", None);
    f.record("alpha", &many(1));
    let gw = f.serve().await;
    let a = gw.get("/api/i/nobody/v1/summary").await;
    assert_eq!(
        (a.status, a.json()["error"].clone()),
        (404, json!("no_such_instance"))
    );
    // a door of a later slice, on a native instance: not served, not a crash
    let a = gw.get("/api/i/alpha/v1/queue").await;
    assert_eq!(
        (a.status, a.json()["error"].clone()),
        (404, json!("not_found"))
    );
    // ids are path segments: nothing walks out of the registry folder
    for p in ["/api/i/..%2F..%2Fx/v1/summary", "/api/i/a%2Fb/v1/record"] {
        let a = gw.get(p).await;
        assert_eq!(a.status, 404, "{p}");
    }
}

#[tokio::test]
async fn both_roles_read_and_the_gateways_guards_apply() {
    let f = Fleet::new("h3-roles");
    f.add("alpha", None);
    f.record("alpha", &story());
    let gw = f.serve().await;
    let doors = [
        "/api/instances",
        "/api/i/alpha/v1/summary",
        "/api/i/alpha/v1/record",
        "/api/i/alpha/v1/record?limit=2&after=1",
    ];
    for d in doors {
        let owner = gw.get(d).await;
        assert_eq!(owner.status, 200, "owner {d}: {}", owner.body);
        let reader = gw.get_as_reader(d).await;
        assert_eq!(reader.status, 200, "reader {d}: {}", reader.body);
        assert_eq!(owner.body, reader.body, "{d}");
        // the token also rides the query (an event stream cannot set headers)
        let q = if d.contains('?') { '&' } else { '?' };
        let by_query = gw.get(&format!("{d}{q}token={READ_TOKEN}")).await;
        assert_eq!(by_query.status, 200, "{d}");
        // an unknown token is refused wherever it is
        let bad = gw
            .send(
                client()
                    .get(format!("{}{d}", gw.base))
                    .bearer_auth("not-a-token"),
            )
            .await;
        assert_eq!(bad.status, 401, "{d}");
        assert_eq!(bad.json()["error"], "unauthorized");
    }
    // the owner role is whoever reaches the gateway: the Host check applies
    let rebound = gw
        .send(
            client()
                .get(format!("{}/api/i/alpha/v1/summary", gw.base))
                .header("host", "evil.example"),
        )
        .await;
    assert_eq!(rebound.status, 403);
    assert_eq!(rebound.json()["error"], "bad_host");
    // a write on a read door: the reader is refused, the owner is told it only reads
    for d in ["/api/i/alpha/v1/summary", "/api/i/alpha/v1/record"] {
        let r = gw
            .send(
                client()
                    .post(format!("{}{d}", gw.base))
                    .bearer_auth(READ_TOKEN),
            )
            .await;
        assert_eq!(
            (r.status, r.json()["error"].clone()),
            (403, json!("read_only")),
            "{d}"
        );
        let o = gw.send(client().post(format!("{}{d}", gw.base))).await;
        assert_eq!(o.status, 405, "{d}");
    }
}

/// An id that is both in the registry and configured is one row, the
/// registry's; the configured instance still serves what the registry cannot.
#[tokio::test]
async fn a_configured_id_in_the_registry_is_one_row() {
    let f = Fleet::new("h3-dup");
    f.add("alpha", None);
    f.record("alpha", &many(2));
    let mut s = f.settings();
    s.instances = vec![rung_gateway::Instance {
        id: "alpha".into(),
        name: "Alpha".into(),
        url: "http://127.0.0.1:9".into(),
        key: OWNER_KEY.into(),
    }];
    let gw = f.serve_with(s).await;
    let rows = gw.get("/api/instances").await.json()["instances"].clone();
    assert_eq!(rows.as_array().unwrap().len(), 1, "{rows}");
    assert_eq!(rows[0]["source"], "registry");
    let p = gw.get("/api/i/alpha/v1/record").await.json();
    assert_eq!(p["total"], 2);
}

// ─── no secret leaves ────────────────────────────────────────────────────────

fn tree(root: &Path) -> BTreeMap<String, (Vec<u8>, std::time::SystemTime)> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, (Vec<u8>, std::time::SystemTime)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(root).unwrap().display().to_string();
            let md = std::fs::metadata(&p).unwrap();
            if md.is_dir() {
                out.insert(format!("{rel}/"), (vec![], md.modified().unwrap()));
                walk(&p, root, out);
            } else {
                out.insert(rel, (std::fs::read(&p).unwrap(), md.modified().unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// A record with secrets in it: a shaped key in an event body, a gateway-held
/// value (an owner key) in a tool result, a named variable's value in an event
/// body, a key shape in the config file, in the config the host started with,
/// and in a waiting queue item.
fn secret_fleet(tag: &str) -> (Fleet, String) {
    let mut f = Fleet::new(tag);
    let cfg = f.root.path().join("alpha.yaml");
    std::fs::write(
        &cfg,
        format!(
            "name: alpha\nstate: /x\nidentity: \"use key {SHAPED} for the router\"\n\
             engine: {{kind: agent, api_key_env: H3_PLAIN_SENTINEL_ENV}}\n\
             headers: {{Authorization: \"Bearer {PLAIN}\"}}\n"
        ),
    )
    .unwrap();
    f.add("alpha", Some(&cfg));
    let mut v = story();
    let n = v.len() as u64;
    v.push(line(n + 1, T0 + 9000, "turn.log", json!({"turn": 2, "header": "h",
        "messages": [{"role": "tool", "content": format!("the router said {SHAPED} and {OWNER_KEY}")}]})));
    v.push(line(
        n + 2,
        T0 + 9001,
        "tool.call",
        json!({"turn": 2, "name": "shell", "group": "core", "ok": true,
        "result": format!("export H3_PLAIN_SENTINEL_ENV={PLAIN}")}),
    ));
    v.push(line(
        n + 3,
        T0 + 9002,
        "stimulus.accepted",
        json!({"item": {"id": "m-3", "kind": "peer",
        "role": "owner", "channel": "owner", "at": T0 + 9002, "firm": false,
        "text": format!("here is my token {SHAPED}")}}),
    ));
    v.push(line(
        n + 4,
        T0 + 9003,
        "note.written",
        json!({"turn": 2, "text": format!("the view token is {READ_TOKEN}")}),
    ));
    // the waiting queue head carries a secret too
    v[4]["item"]["text"] = json!(format!("my key is {SHAPED}"));
    f.record("alpha", &v);
    f.hold("alpha");
    (f, cfg.display().to_string())
}

#[tokio::test]
async fn no_secret_ever_appears_in_any_door() {
    // SAFETY: a variable only this test file reads, set before the gateway starts.
    unsafe { std::env::set_var("H3_PLAIN_SENTINEL_ENV", PLAIN) };
    let (f, _) = secret_fleet("h3-secret");
    let mut s = f.settings();
    s.instances = vec![rung_gateway::Instance {
        id: "other".into(),
        name: "Other".into(),
        url: "http://127.0.0.1:9".into(),
        key: OWNER_KEY.into(),
    }];
    let gw = f.serve_with(s).await;

    let doors = [
        "/api/instances",
        "/api/i/alpha/v1/summary",
        "/api/i/alpha/v1/record",
        "/api/i/alpha/v1/record?order=desc",
        "/api/i/alpha/v1/record?limit=1&after=9",
        "/api/i/alpha/v1/record?limit=1&offset=10",
        "/api/i/alpha/v1/record?limit=3&offset=11&order=asc",
    ];
    for d in doors {
        for (who, a) in [
            ("owner", gw.get(d).await),
            ("reader", gw.get_as_reader(d).await),
        ] {
            assert_eq!(a.status, 200, "{who} {d}: {}", a.body);
            for secret in [SHAPED, PLAIN, OWNER_KEY, READ_TOKEN] {
                assert!(
                    !a.body.contains(secret),
                    "{who} {d} leaked {secret}: {}",
                    a.body
                );
                // nor in the JSON-escaped form a tool's input has
                assert!(!a.body.contains(&secret.replace('"', "\\\"")), "{who} {d}");
            }
        }
    }
    // it was redacted on the way out, not lost: the lines and the queue head are still there
    let p = gw.get("/api/i/alpha/v1/record?limit=1000").await.json();
    let body = p.to_string();
    assert!(body.contains("[redacted]"), "{body}");
    assert!(body.contains("the router said"));
    let s = gw.get("/api/i/alpha/v1/summary").await.json();
    assert_eq!(s["queue"]["head"]["id"], "m-1");
    assert!(
        s["queue"]["head"]["text"]
            .as_str()
            .unwrap()
            .starts_with("my key is ")
    );
    assert!(
        !s["queue"]["head"]["text"]
            .as_str()
            .unwrap()
            .contains(SHAPED)
    );
    assert!(
        s["config"]["file"]["content"]["identity"]
            .as_str()
            .unwrap()
            .contains("[redacted]")
    );
    // the record on disk stays verbatim: the cached prompt is rebuilt from it
    let disk =
        std::fs::read_to_string(f.state("alpha").join("record/seg-00000001.ndjson")).unwrap();
    assert!(disk.contains(SHAPED) && disk.contains(PLAIN));
}

#[tokio::test]
async fn reading_leaves_every_byte_of_the_state_where_it_was() {
    let (f, _) = secret_fleet("h3-readonly");
    // a stopped one beside it, and a corrupt one
    f.add("beta", None);
    f.record(
        "beta",
        &numbered(vec![
            ("host.start", json!({"config": {}})),
            ("halted", json!({"why": "x"})),
        ]),
    );
    f.add("bad", None);
    std::fs::create_dir_all(f.state("bad").join("record")).unwrap();
    std::fs::write(f.state("bad").join("record/seg-00000001.ndjson"), "junk\n").unwrap();
    let before = tree(f.root.path());
    let gw = f.serve().await;
    for d in [
        "/api/instances",
        "/api/i/alpha/v1/summary",
        "/api/i/alpha/v1/record?limit=2",
        "/api/i/alpha/v1/record?order=desc",
        "/api/i/beta/v1/summary",
        "/api/i/beta/v1/record",
        "/api/i/bad/v1/summary",
        "/api/i/bad/v1/record",
    ] {
        assert_eq!(gw.get(d).await.status, 200, "{d}");
        assert_eq!(gw.get_as_reader(d).await.status, 200, "{d}");
    }
    let after = tree(f.root.path());
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "no file made or removed"
    );
    for (k, (bytes, mtime)) in &before {
        assert_eq!(bytes, &after[k].0, "{k} changed");
        assert_eq!(mtime, &after[k].1, "{k} touched");
    }
}

/// A live host's lock is not disturbed by being read: the lock the test
/// holds is still held after every door has answered.
#[tokio::test]
async fn reading_does_not_take_the_instances_lock() {
    let (f, _) = secret_fleet("h3-lock");
    let gw = f.serve().await;
    for d in [
        "/api/instances",
        "/api/i/alpha/v1/summary",
        "/api/i/alpha/v1/record",
    ] {
        gw.get(d).await;
    }
    match StateLock::acquire(&f.state("alpha")) {
        Err(rung_host::statelock::LockError::Held { .. }) => {}
        other => panic!("the lock should still be held, got {other:?}"),
    }
}
