//! R1 of the fleet: two chiefs and a steward, three real hosts on one machine
//! behind one gateway, watched through the read-only role.
//!
//! No fleet code is involved: each member is an ordinary `rung-host` started
//! from its own config (mock engine, no network, no key), registered in the
//! instance registry under a name that begins with its role. The gateway lists
//! them from the registry, a phone-style client holding only the read-only
//! token lists them, reads a record and follows a live stream, and an owner
//! message dropped in one chief's inbox is seen on that chief's stream and
//! nowhere else.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rung_gateway::Settings;
use rung_host::registry::{self, Word};
use rung_host::startup;
use rung_testkit::TempDir;
use serde_json::{Value, json};

const VIEW_TOKEN: &str = "view-token-canary-r1-7731";
const GUARD: Duration = Duration::from_secs(30);

/// The fleet's members and what each one is told. The first segment of an
/// instance name is its role; the rest says which session or project.
const MEMBERS: [&str; 3] = ["chief-main", "chief-second", "steward-alpha"];

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

struct Member {
    name: &'static str,
    dir: PathBuf,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Member {
    fn state(&self) -> PathBuf {
        self.dir.join("state")
    }
    fn inbox(&self) -> PathBuf {
        self.dir.join("inbox")
    }
    fn stop_file(&self) -> PathBuf {
        self.dir.join("STOP")
    }

    /// Write this member's config and start its host on its own thread.
    fn start(root: &Path, name: &'static str) -> Member {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("inbox")).unwrap();
        std::fs::create_dir_all(dir.join("workspace")).unwrap();
        let cfg = dir.join("rung-host.yaml");
        std::fs::write(
            &cfg,
            format!(
                "name: {name}\nstate: {d}/state\nworkspace: {d}/workspace\n\
                 inbox: {d}/inbox\nstop_file: {d}/STOP\nengine:\n  kind: mock\nmemory: false\n",
                d = dir.display()
            ),
        )
        .unwrap();
        let handoff = startup::start(startup::registered(
            startup::configure(&cfg, None).expect("a valid member config"),
        ))
        .expect("the member starts");
        let thread = std::thread::spawn(move || {
            let _ = handoff.run();
        });
        Member {
            name,
            dir,
            thread: Some(thread),
        }
    }
}

/// Everything under `dir` (records, inbox, memory), as one string.
fn all_text(dir: &Path) -> String {
    let mut out = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(t) = std::fs::read_to_string(&p) {
                out.push_str(&t);
            }
        }
    }
    out
}

async fn get(base: &str, path: &str, token: Option<&str>) -> (u16, Value) {
    let mut rb = client().get(format!("{base}{path}"));
    if let Some(t) = token {
        rb = rb.bearer_auth(t);
    }
    let r = rb.send().await.unwrap();
    (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
}

/// Read server-sent frames from `resp` until a record frame satisfies `want`.
async fn watch_for(resp: &mut reqwest::Response, want: impl Fn(&Value) -> bool) -> Value {
    let end = Instant::now() + GUARD;
    let mut buf = String::new();
    loop {
        while let Some(i) = buf.find("\n\n") {
            let block: String = buf.drain(..i + 2).collect();
            if !block.contains("event: record") {
                continue;
            }
            let data: String = block
                .lines()
                .filter_map(|l| l.strip_prefix("data: "))
                .collect();
            let v: Value = serde_json::from_str(&data).unwrap();
            if want(&v) {
                return v;
            }
        }
        let left = end.saturating_duration_since(Instant::now());
        let chunk = tokio::time::timeout(left, resp.chunk())
            .await
            .expect("the live stream never showed the line")
            .unwrap()
            .expect("the stream ended");
        buf.push_str(std::str::from_utf8(&chunk).unwrap());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_chiefs_and_a_steward_behind_one_gateway() {
    let root = TempDir::new("fleet-r1");
    let home = root.path().join("home");
    // The members register themselves: keep that out of the operator's home.
    // Set before any member thread exists.
    unsafe { std::env::set_var("RUNG_HOME", &home) };

    let mut members: Vec<Member> = MEMBERS
        .iter()
        .map(|n| Member::start(root.path(), n))
        .collect();

    let gw = rung_gateway::start(Settings {
        listen: "127.0.0.1:0".parse().unwrap(),
        app_dir: None,
        allowed_hosts: vec![],
        read_only_tokens: vec![VIEW_TOKEN.into()],
        registry_dir: Some(registry::dir()),
        instances: vec![],
    })
    .await
    .unwrap();
    let base = format!("http://{}", gw.addr);

    // ── the phone lists the fleet: three members, each named by its role ──
    let (s, list) = get(&base, "/api/instances", Some(VIEW_TOKEN)).await;
    assert_eq!(s, 200);
    let rows = list["instances"].as_array().unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, MEMBERS, "one row per member, ordered by id");
    for r in rows {
        assert_eq!(r["word"], "running", "{}", r["id"]);
        assert_eq!(r["source"], "registry");
        let role = r["id"].as_str().unwrap().split('-').next().unwrap();
        assert!(["chief", "steward"].contains(&role), "{}", r["id"]);
    }
    assert_eq!(ids.iter().filter(|i| i.starts_with("chief")).count(), 2);

    // each member has its own state, inbox and record
    let states: std::collections::BTreeSet<_> = rows
        .iter()
        .map(|r| r["state_dir"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(states.len(), 3, "no two members share a state directory");

    // ── the phone reads a steward's record ──
    let (s, rec) = get(
        &base,
        "/api/i/steward-alpha/v1/record?limit=5",
        Some(VIEW_TOKEN),
    )
    .await;
    assert_eq!(s, 200);
    let lines = rec["lines"].as_array().unwrap();
    assert!(!lines.is_empty(), "the steward's record has lines");
    assert_eq!(lines[0]["seq"], 1);

    // ── and watches a chief live, then an owner message arrives ──
    let (_, sum) = get(&base, "/api/i/chief-main/v1/summary", Some(VIEW_TOKEN)).await;
    let after = sum["last_seq"].as_u64().unwrap();
    let mut live = client()
        .get(format!("{base}/api/i/chief-main/v1/events?after={after}"))
        .bearer_auth(VIEW_TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(live.status(), 200);
    let marker = "marker-for-the-first-chief-r1-0001";
    let msg = json!({"role": "owner", "text": marker});
    std::fs::write(members[0].inbox().join("hello.msg"), msg.to_string()).unwrap();
    let seen = watch_for(&mut live, |v| {
        v["kind"] == "stimulus.accepted" && v.to_string().contains(marker)
    })
    .await;
    assert!(seen["seq"].as_u64().unwrap() > after);

    // ── contexts never mix: the marker is in the first chief's state and in
    //    no other member's state, inbox or pack ──
    assert!(all_text(&members[0].state()).contains(marker));
    for other in &members[1..] {
        assert!(
            !all_text(&other.dir).contains(marker),
            "{} saw the first chief's message",
            other.name
        );
    }

    // ── the read-only role observes and cannot write ──
    let w = client()
        .post(format!("{base}/api/i/chief-main/v1/queue"))
        .bearer_auth(VIEW_TOKEN)
        .json(&json!({"text": "do something"}))
        .send()
        .await
        .unwrap();
    assert_eq!(w.status(), 403);
    assert_eq!(w.json::<Value>().await.unwrap()["error"], "read_only");
    let (s, _) = get(&base, "/api/instances", Some("not-the-token")).await;
    assert_eq!(s, 401);

    // ── stopping a member keeps it listed, as `stopped` ──
    for m in &members {
        std::fs::write(m.stop_file(), "").unwrap();
    }
    for m in &mut members {
        m.thread.take().unwrap().join().unwrap();
    }
    let rows = registry::list().unwrap();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| r.word == Word::Stopped));
    let (_, list) = get(&base, "/api/instances", Some(VIEW_TOKEN)).await;
    assert_eq!(list["instances"].as_array().unwrap().len(), 3);
}
