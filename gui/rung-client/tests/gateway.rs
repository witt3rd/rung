//! The client against the real `rung-gateway` binary in front of two real
//! `rung-host` processes (a chief and a steward, mock engine: no network, no
//! key), with the read-only role.
//!
//! The binaries come from the main workspace, which this one does not build:
//! build them first (`cargo build -p rung-gateway -p rung-host` at the
//! repository root) or point `RUNG_BIN_DIR` at a directory that holds them.
//! A missing binary fails the test with that instruction; it never skips.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rung_client::{Client, Error, Role};

const TOKEN_ENV: &str = "RUNG_CLIENT_TEST_VIEW_TOKEN";
const TOKEN: &str = "view-token-canary-client-4417";
const GUARD: Duration = Duration::from_secs(30);

fn bin(name: &str) -> PathBuf {
    let dir = std::env::var_os("RUNG_BIN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug"));
    let p = dir.join(name);
    assert!(
        p.exists(),
        "{} is missing: run `cargo build -p rung-gateway -p rung-host` at the repository root, or set RUNG_BIN_DIR",
        p.display()
    );
    p
}

/// A scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    /// Unique per call: the tests of this file run side by side.
    fn new() -> Scratch {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rung-client-gw-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A child this test started; killed on drop.
struct Proc(Child);

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_host(root: &Path, home: &Path, name: &str) -> Proc {
    start_host_with(root, home, name, "", &[])
}

/// A host whose config has `extra` lines appended and `envs` set.
fn start_host_with(
    root: &Path,
    home: &Path,
    name: &str,
    extra: &str,
    envs: &[(&str, &str)],
) -> Proc {
    let dir = root.join(name);
    std::fs::create_dir_all(dir.join("inbox")).unwrap();
    std::fs::create_dir_all(dir.join("workspace")).unwrap();
    let cfg = dir.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "name: {name}\nstate: {d}/state\nworkspace: {d}/workspace\n\
             inbox: {d}/inbox\nstop_file: {d}/STOP\nengine:\n  kind: mock\nmemory: false\nidle_time_s: 0\n{extra}",
            d = dir.display()
        ),
    )
    .unwrap();
    Proc(
        Command::new(bin("rung-host"))
            .args(["run", "--config"])
            .arg(&cfg)
            .envs(envs.iter().copied())
            .env("RUNG_HOME", home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start rung-host"),
    )
}

/// Start the gateway on an OS-chosen port; read the address it prints.
fn start_gateway(root: &Path, home: &Path) -> (Proc, String) {
    start_gateway_with(root, home, "", &[])
}

/// A gateway whose config has `extra` lines appended (an `instances:` block)
/// and `envs` set (the variables that block names).
fn start_gateway_with(
    root: &Path,
    home: &Path,
    extra: &str,
    envs: &[(&str, &str)],
) -> (Proc, String) {
    let cfg = root.join("gateway.yaml");
    std::fs::write(
        &cfg,
        format!(
            "listen: 127.0.0.1:0\nregistry_dir: {}\nread_only_token_envs: [{TOKEN_ENV}]\n{extra}",
            home.join("instances").display()
        ),
    )
    .unwrap();
    let mut child = Command::new(bin("rung-gateway"))
        .args(["--config"])
        .arg(&cfg)
        .env(TOKEN_ENV, TOKEN)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start rung-gateway");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let addr = line
        .split("http://")
        .nth(1)
        .and_then(|r| r.split_whitespace().next())
        .unwrap_or_else(|| panic!("the gateway printed no address: {line:?}"))
        .to_string();
    (Proc(child), format!("http://{addr}"))
}

#[test]
fn a_chief_and_a_steward_behind_a_real_gateway() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    let _chief = start_host(&scratch.0, &home, "chief-main");
    let _steward = start_host(&scratch.0, &home, "steward-alpha");
    let (_gw, base) = start_gateway(&scratch.0, &home);

    let client = Client::new(&base).unwrap().with_token(TOKEN);
    assert!(client.health().expect("health").ok);

    // the hosts register themselves as they start: wait until both read
    let end = Instant::now() + GUARD;
    let list = loop {
        let list = client.instances().expect("the list");
        if list.instances.len() == 2
            && list.instances.iter().all(|i| {
                // a host's first lines are its start: wait for them
                i.summary.as_ref().is_some_and(|s| {
                    s.desk.as_ref().is_some_and(|d| d.mode.is_some())
                        && s.governor.as_ref().is_some_and(|g| g.model.is_some())
                })
            })
        {
            break list;
        }
        assert!(
            Instant::now() < end,
            "the hosts never showed in the list: {list:?}"
        );
        std::thread::sleep(Duration::from_millis(200));
    };

    assert_eq!(list.registry_error, None);
    let ids: Vec<&str> = list.instances.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["chief-main", "steward-alpha"], "ordered by id");
    let roles: Vec<Role> = list.instances.iter().map(|i| i.role()).collect();
    assert_eq!(
        roles,
        [Role::Chief, Role::Steward],
        "the role is read from the id"
    );
    for i in &list.instances {
        assert_eq!(i.word.as_deref(), Some("running"), "{}", i.id);
        assert_eq!(i.source.as_deref(), Some("registry"));
        assert!(i.reachable && !i.unreachable());
        // the words are the host's own, not the client's
        let s = i.summary.as_ref().unwrap();
        assert_eq!(i.state_word(), s.state.as_deref().unwrap());
        assert!(!i.state_word().is_empty());
        // the page's sections come from the host's own summary
        let desk = s.desk.as_ref().expect("a desk");
        assert_eq!(desk.mode.as_deref(), Some("rule_only"), "{}", i.id);
        assert!(desk.limits.as_ref().and_then(|l| l.cap_usd_day).is_some());
        let gov = s.governor.as_ref().expect("a governor");
        assert!(gov.model.is_some() && !gov.ladder.is_empty(), "{}", i.id);
        assert!(s.queue.is_some() && s.requests.is_some() && s.spend_usd_day.is_some());
        assert!(!i.doing().is_empty(), "{}: a host says what it does", i.id);
    }
}

#[test]
fn a_wrong_token_is_refused_in_the_gateways_words() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    let (_gw, base) = start_gateway(&scratch.0, &home);
    // no token reads as the owner (the tailnet is the wall); a wrong one does not
    assert!(Client::new(&base).unwrap().health().unwrap().ok);
    let e = Client::new(&base)
        .unwrap()
        .with_token("not-the-token")
        .health()
        .unwrap_err();
    assert!(
        matches!(
            e,
            Error::Status {
                code: 401 | 403,
                ..
            }
        ),
        "{e}"
    );
    assert!(!e.to_string().contains("not-the-token"));
}

/// Read frames from `events` until `want` says it has seen enough.
fn read_until(
    events: rung_client::Events,
    mut want: impl FnMut(&rung_client::Frame) -> bool,
) -> Vec<rung_client::Frame> {
    let end = Instant::now() + GUARD;
    let mut got = Vec::new();
    for f in events {
        let f = f.expect("a readable frame");
        let done = want(&f);
        got.push(f);
        if done || Instant::now() > end {
            break;
        }
    }
    got
}

#[test]
fn the_stream_of_a_real_host_replays_follows_and_resumes_by_cursor() {
    use rung_client::Frame;
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    let _chief = start_host(&scratch.0, &home, "chief-main");
    let (_gw, base) = start_gateway(&scratch.0, &home);
    let client = Client::new(&base).unwrap().with_token(TOKEN);
    let end = Instant::now() + GUARD;
    while client.instances().unwrap().instances.is_empty() {
        assert!(Instant::now() < end, "the host never listed");
        std::thread::sleep(Duration::from_millis(100));
    }

    // from the first line: an open frame, then records 1, 2, 3, ... in order
    let first = read_until(
        client.events("chief-main", None).unwrap(),
        |f| matches!(f, Frame::Record { seq, .. } if *seq >= 6),
    );
    assert!(
        matches!(&first[0], Frame::Open(o) if o.after == 0 && o.live),
        "{:?}",
        first[0]
    );
    let seqs: Vec<u64> = first
        .iter()
        .filter_map(|f| match f {
            Frame::Record { seq, line } => {
                assert_eq!(line["seq"], *seq, "the id is the line's own number");
                Some(*seq)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        seqs,
        (1..=6).collect::<Vec<u64>>(),
        "every line once, in order"
    );

    // resume: the next line after the cursor, none repeated
    let again = read_until(
        client.events("chief-main", Some(4)).unwrap(),
        |f| matches!(f, Frame::Record { seq, .. } if *seq >= 6),
    );
    assert!(matches!(&again[0], Frame::Open(o) if o.after == 4));
    let resumed: Vec<u64> = again
        .iter()
        .filter_map(|f| match f {
            Frame::Record { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    assert_eq!(resumed, [5, 6]);

    // a cursor past the record is the gateway's 409
    let e = client
        .events("chief-main", Some(u32::MAX as u64))
        .err()
        .unwrap();
    assert!(matches!(e, Error::Status { code: 409, .. }), "{e}");
    // an unknown instance is a 404
    let e = client.events("chief-nobody", None).err().unwrap();
    assert!(matches!(e, Error::Status { code: 404, .. }), "{e}");
}

#[test]
fn a_host_that_goes_away_ends_its_stream_down_and_follow_reconnects_to_it() {
    use rung_client::{Event, Follow, Frame};
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    let steward = start_host(&scratch.0, &home, "steward-alpha");
    let (_gw, base) = start_gateway(&scratch.0, &home);
    let client = Client::new(&base).unwrap().with_token(TOKEN);

    let opts = Follow {
        first: Duration::from_millis(100),
        max: Duration::from_millis(400),
        healthy: Duration::from_secs(3600),
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    let mut steward = Some(steward);
    let (mut seen, mut last_before_kill, mut ended) = (0u64, 0u64, false);
    let deadline = Instant::now() + GUARD;
    rung_client::follow(&client, "steward-alpha", None, opts, &stop, |e| {
        assert!(
            Instant::now() < deadline,
            "the follow never saw the host go"
        );
        match e {
            Event::Frame(Frame::Record { seq, .. }) => {
                assert_eq!(seq, seen + 1, "no line missed or repeated");
                seen = seq;
                if steward.is_some() && seq >= 3 {
                    // kill the host this test started
                    drop(steward.take());
                    last_before_kill = seq;
                }
            }
            Event::Frame(Frame::End(end)) => {
                assert_eq!(end.reason, "down", "a killed host is down, not stopped");
                assert!(end.last_seq.unwrap_or(0) >= last_before_kill);
                ended = true;
                // the host is gone for good: stop following
                stop.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            _ => {}
        }
        true
    });
    assert!(ended, "an `end` frame came");
    assert!(seen >= 3);
}

/// A port nothing listens on now (the host's own door needs one the gateway's
/// config can name before the host starts).
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn the_owner_controls_work_through_a_real_gateway_and_the_read_only_token_cannot() {
    use rung_client::Intent;
    const OWNER_ENV: &str = "RUNG_CLIENT_TEST_OWNER_KEY";
    const OWNER_KEY: &str = "owner-key-canary-client-9902";
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    let port = free_port();
    let _chief = start_host_with(
        &scratch.0,
        &home,
        "chief-main",
        &format!("acp:\n  http: 127.0.0.1:{port}\n  tokens:\n    owner: {OWNER_ENV}\n"),
        &[(OWNER_ENV, OWNER_KEY)],
    );
    // the gateway holds the write key by name, for this instance
    let (_gw, base) = start_gateway_with(
        &scratch.0,
        &home,
        &format!(
            "instances:\n  - id: chief-main\n    url: http://127.0.0.1:{port}\n    key_env: {OWNER_ENV}\n"
        ),
        &[(OWNER_ENV, OWNER_KEY)],
    );
    let owner = Client::new(&base).unwrap(); // no token: the owner role
    let viewer = Client::new(&base).unwrap().with_token(TOKEN);
    let end = Instant::now() + GUARD;
    while owner.queue("chief-main", None).is_err() || owner.health().is_err() {
        assert!(Instant::now() < end, "the host never came up");
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(500));

    // send: the key becomes the item's id; a retry with the same key adds once
    let a = owner
        .queue_add(
            "chief-main",
            "please add the appendix",
            Intent::Steer,
            "ui-test-1",
        )
        .expect("the owner may send");
    assert_eq!((a.id.as_str(), a.repeat), ("ui-test-1", false));
    let again = owner
        .queue_add(
            "chief-main",
            "please add the appendix",
            Intent::Steer,
            "ui-test-1",
        )
        .unwrap();
    assert!(again.repeat, "the same key is one item");
    let all = owner.queue("chief-main", Some("all")).unwrap();
    let item = all
        .items
        .iter()
        .find(|i| i.id == "ui-test-1")
        .expect("listed");
    assert_eq!(
        (item.role.as_str(), item.intent.as_deref()),
        ("owner", Some("steer"))
    );
    assert_eq!(item.text, "please add the appendix");
    assert!(["waiting", "answering", "answered"].contains(&item.state.as_str()));

    // a cancel of an item that is not there is the gateway's own refusal in words
    let e = owner
        .queue_cancel("chief-main", "no-such-item")
        .unwrap_err();
    assert!(matches!(e, Error::Status { code: 404, .. }), "{e}");
    // a move of one that is not there likewise
    let e = owner
        .queue_move("chief-main", "no-such-item", None, "mv-x")
        .unwrap_err();
    assert!(matches!(e, Error::Status { code: 404, .. }), "{e}");

    // the read-only token reads the queue and nothing more, in the gateway's words
    assert!(viewer.queue("chief-main", None).is_ok());
    let e = viewer
        .queue_add("chief-main", "x", Intent::Next, "ui-test-2")
        .unwrap_err();
    assert_eq!(
        e,
        Error::Status {
            code: 403,
            message: "this token can only read".into()
        },
        "{e}"
    );
    assert!(matches!(
        viewer.stop("chief-main"),
        Err(Error::Status { code: 403, .. })
    ));
    assert!(matches!(
        viewer.release("chief-main", None),
        Err(Error::Status { code: 403, .. })
    ));
    assert!(!e.to_string().contains(TOKEN) && !e.to_string().contains(OWNER_KEY));

    // release with nothing held changes nothing; stop is acknowledged, then repeats
    let r = owner.release("chief-main", Some("test")).unwrap();
    assert!(r.released || r.repeat);
    let s = owner.stop("chief-main").unwrap();
    assert!(s.stopping && !s.repeat);
    let s2 = owner.stop("chief-main");
    assert!(
        matches!(&s2, Ok(s) if s.repeat) || matches!(&s2, Err(Error::Status { code: 409, .. })),
        "{s2:?}"
    );
}

#[test]
fn the_context_of_a_real_turn_is_read_as_bands_and_refusals_are_the_hosts_words() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    let _chief = start_host(&scratch.0, &home, "chief-main");
    let (_gw, base) = start_gateway(&scratch.0, &home);
    let client = Client::new(&base).unwrap().with_token(TOKEN);
    // wait for a few finished turns
    let end = Instant::now() + GUARD;
    let turn = loop {
        let t = client
            .instances()
            .ok()
            .and_then(|l| l.instances.first().and_then(|i| i.summary.as_ref()?.turn));
        if let Some(t) = t.filter(|t| *t >= 3) {
            break t - 1; // the one before the running turn has ended
        }
        assert!(Instant::now() < end, "the host never ran three turns");
        std::thread::sleep(Duration::from_millis(100));
    };
    let v = client.context("chief-main", turn, None).expect("a context");
    assert_eq!((v.turn, v.call), (turn, 1));
    assert!(v.calls >= 1 && !v.live && v.recorded, "{v:?}");
    let order: Vec<&str> = v.bands.iter().map(|b| b.band.as_str()).collect();
    assert_eq!(
        &order[..3],
        ["tools", "system", "epoch"],
        "in the order the model sees them"
    );
    assert_eq!(order.last(), Some(&"this_turn"));
    assert!(
        v.bands
            .iter()
            .all(|b| b.est_tokens > 0 || b.band == "this_turn")
    );
    let sum: u64 = v.bands.iter().map(|b| b.est_tokens).sum();
    assert_eq!(sum, v.estimate_tokens, "the host's own sum");
    let p = v.provider.as_ref().expect("the provider's counts");
    assert!(p.prompt_tokens > 0 && p.cached_tokens <= p.prompt_tokens);
    assert!(v.bands[0].count.unwrap_or(0) as usize == v.bands[0].items.len());
    assert!(
        v.bands[1].text.as_deref().is_some_and(|t| !t.is_empty()),
        "the system text, whole"
    );
    assert!(
        v.bands.iter().all(|b| b.cache.is_some()),
        "every band has a cache mark"
    );

    let e = client.context("chief-main", 99_999, None).unwrap_err();
    assert!(
        matches!(&e, Error::Status { code: 404, message } if !message.is_empty()),
        "{e}"
    );
    let e = client.context("chief-main", turn, Some(999)).unwrap_err();
    assert!(matches!(e, Error::Status { code: 404, .. }), "{e}");
    assert!(matches!(
        client.context("a/b", 1, None),
        Err(Error::BadId(_))
    ));
}

#[test]
fn the_turns_of_a_real_host_are_listed_newest_first_and_read_whole() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    let _chief = start_host(&scratch.0, &home, "chief-main");
    let (_gw, base) = start_gateway(&scratch.0, &home);
    let client = Client::new(&base).unwrap().with_token(TOKEN);
    let end = Instant::now() + GUARD;
    let list = loop {
        if let Ok(l) = client.turns("chief-main", 3, None)
            && l.total >= 5
        {
            break l;
        }
        assert!(Instant::now() < end, "the host never ran five turns");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(list.turns.len(), 3);
    let nums: Vec<u64> = list.turns.iter().map(|t| t.turn).collect();
    assert!(
        nums.windows(2).all(|w| w[0] == w[1] + 1),
        "newest first, no gap: {nums:?}"
    );
    assert_eq!(
        list.next,
        Some(*nums.last().unwrap()),
        "next is the cursor for older turns"
    );
    // the older page, by that cursor, starts right below it and repeats nothing
    let older = client.turns("chief-main", 2, list.next).unwrap();
    assert_eq!(older.turns[0].turn, nums.last().unwrap() - 1);
    // a finished turn: its facts and its reply
    let done = list
        .turns
        .iter()
        .find(|t| t.status == "completed")
        .expect("a finished turn");
    assert!(
        done.text.starts_with(&format!("turn {} done", done.turn)),
        "{}",
        done.text
    );
    assert!(done.llm.calls >= 1 && done.llm.prompt_tokens > 0);
    let one = client.turn("chief-main", done.turn).unwrap();
    assert_eq!(one.row.text, done.text, "the same reply, whole");
    assert_eq!(one.calls.len() as u64, one.row.tool_calls);
    assert!(one.calls.iter().all(|c| !c.name.is_empty()));
    // refusals are the door's own words
    let e = client.turn("chief-main", 99_999).unwrap_err();
    assert!(
        matches!(&e, Error::Status { code: 404, message } if message.contains("no turn")),
        "{e}"
    );
    assert!(matches!(client.turns("a/b", 3, None), Err(Error::BadId(_))));
}
