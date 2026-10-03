//! G-h (stop and the watchdog) and G-i (`kill -9` restarts), against the
//! `rung-host` binary on the real clock with the mock engine. G-k over
//! every record these runs wrote.

mod common;

use std::collections::BTreeSet;
use std::os::unix::net::UnixDatagram;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common::*;
use rung_host::gates::{self, StopCase};
use rung_host::record::{Line, Record};
use rung_host::sim::{self, Rng};
use rung_host::state::State;

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn spawn(state: &Path, extra: &[&str], env: &[(&str, String)]) -> Child {
    let mut c = Command::new(BIN);
    c.args(["sim", "--clock", "real", "--state", state.to_str().unwrap()])
        .args(extra);
    for (k, v) in env {
        c.env(k, v);
    }
    c.stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn rung-host")
}

fn lines(state: &Path) -> Vec<Line> {
    Record::read_dir(state.join("record")).unwrap_or_default()
}

/// Poll until `f` holds over the record, or panic after `secs`.
fn wait_for(state: &Path, secs: u64, what: &str, f: impl Fn(&[Line]) -> bool) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(secs) {
        if f(&lines(state)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for {what}");
}

fn sigterm(c: &Child) {
    // SAFETY: sending a signal to our own child.
    unsafe {
        libc::kill(c.id() as libc::pid_t, libc::SIGTERM);
    }
}

/// SIGTERM, then the time until exit and the exit code.
fn stop(mut c: Child) -> (u64, Option<i32>) {
    sigterm(&c);
    let t = Instant::now();
    loop {
        if let Some(s) = c.try_wait().unwrap() {
            return (t.elapsed().as_millis() as u64, s.code());
        }
        if t.elapsed() > Duration::from_secs(60) {
            let _ = c.kill();
            return (t.elapsed().as_millis() as u64, None);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn halted(state: &Path) -> bool {
    lines(state)
        .last()
        .is_some_and(|l| l.kind == "halted" && l.get("why")["why"] == "stopped")
}

/// (turn, call, start ms, duration ms) per mock call.
fn calls(state: &Path) -> Vec<(u64, u64, i64, i64)> {
    std::fs::read_to_string(state.join("mock-calls.log"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let v: Vec<i64> = l
                .split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect();
            (v.len() == 4).then(|| (v[0] as u64, v[1] as u64, v[2], v[3]))
        })
        .collect()
}

fn mid_turn() -> StopCase {
    let dir = sim::temp_dir("gate-h-turn");
    let c = spawn(&dir, &["--call-ms", "1500,1500", "--no-memory"], &[]);
    wait_for(&dir, 60, "a call in flight", |_| calls(&dir).len() >= 2);
    std::thread::sleep(Duration::from_millis(700));
    let sent = now_ms();
    let (elapsed, code) = stop(c);
    let in_flight = calls(&dir)
        .into_iter()
        .rfind(|(_, _, s, _)| *s <= sent)
        .map(|(_, _, s, d)| (s + d - sent).max(0) as u64)
        .unwrap_or(0);
    StopCase {
        case: "mid_turn".into(),
        elapsed_ms: elapsed,
        remaining_call_ms: in_flight,
        exit_code: code,
        halted_recorded: halted(&dir),
    }
}

fn in_wait(case: &str, args: &[&str], class: &str) -> StopCase {
    let dir = sim::temp_dir(&format!("gate-h-{case}"));
    let c = spawn(&dir, args, &[]);
    let class = class.to_string();
    wait_for(&dir, 60, case, |ls| {
        ls.iter()
            .any(|l| l.kind == "degraded" && l.str("class") == class)
    });
    std::thread::sleep(Duration::from_millis(200));
    let (elapsed, code) = stop(c);
    StopCase {
        case: case.into(),
        elapsed_ms: elapsed,
        remaining_call_ms: 0,
        exit_code: code,
        halted_recorded: halted(&dir),
    }
}

/// A wedged engine: the harness, as supervisor, watches `WATCHDOG=1`.
fn wedged(watchdog_ms: u64) -> Option<u64> {
    let dir = sim::temp_dir("gate-h-wedged");
    let sock_path = dir.join("notify.sock");
    let sock = UnixDatagram::bind(&sock_path).unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let mut c = spawn(
        &dir,
        &["--call-ms", "20,40", "--wedge-at", "8", "--no-memory"],
        &[
            ("NOTIFY_SOCKET", sock_path.display().to_string()),
            ("WATCHDOG_USEC", (watchdog_ms * 1000).to_string()),
        ],
    );
    let mut buf = [0u8; 256];
    let mut last: Option<Instant> = None;
    let mut pings = 0;
    let started = Instant::now();
    let detected = loop {
        if let Ok(n) = sock.recv(&mut buf)
            && &buf[..n] == b"WATCHDOG=1"
        {
            last = Some(Instant::now());
            pings += 1;
        }
        if let Some(t) = last
            && t.elapsed() > Duration::from_millis(watchdog_ms)
        {
            break Some(t.elapsed().as_millis() as u64);
        }
        if started.elapsed() > Duration::from_secs(60) {
            break None;
        }
    };
    let _ = c.kill();
    let _ = c.wait();
    // A healthy loop pinged before it wedged.
    assert!(pings >= 1, "no watchdog pings before the wedge");
    // It wedged at the scripted turn, not before.
    let started_turns = lines(&dir)
        .iter()
        .filter(|l| l.kind == "turn.started")
        .count();
    assert!(started_turns >= 8, "wedged early: {started_turns} turns");
    detected
}

#[test]
fn a_stop_is_prompt_from_a_turn_and_from_any_wait_and_the_watchdog_fires() {
    sim::test_timeout(600);
    let cases = vec![
        mid_turn(),
        in_wait(
            "backoff",
            &[
                "--call-ms",
                "10,10",
                "--fault",
                "outage",
                "--backoff-ms",
                "30000",
                "--no-memory",
            ],
            "backoff",
        ),
        in_wait(
            "paced",
            &["--call-ms", "10,10", "--quota", "1000,1", "--no-memory"],
            "paced",
        ),
    ];
    let watchdog_ms = 3_000;
    let detect = wedged(watchdog_ms);
    assert_gate(&gates::g_h(&cases, detect, watchdog_ms));
}

#[test]
fn fifty_kills_lose_nothing_and_restore_everything() {
    sim::test_timeout(900);
    let dir = sim::temp_dir("gate-i");
    let inbox = dir.join("inbox");
    std::fs::create_dir_all(&inbox).unwrap();
    let args = ["--call-ms", "5,25", "--inbox", inbox.to_str().unwrap()];
    let mut rng = Rng::new(50);
    let mut sent = 0;
    let mut write_msgs = |n: usize, rng: &mut Rng| {
        for _ in 0..n {
            sent += 1;
            let role = if rng.below(3) == 0 { "owner" } else { "peer" };
            let body = serde_json::json!({"role": role, "text": format!("message {sent}")});
            let tmp = inbox.join(format!(".m{sent:04}.tmp"));
            std::fs::write(&tmp, body.to_string()).unwrap();
            std::fs::rename(&tmp, inbox.join(format!("m{sent:04}.msg"))).unwrap();
        }
    };
    let mut kills = 0;
    for _ in 0..gates::G_I_KILLS {
        write_msgs(3, &mut rng);
        let mut c = spawn(&dir, &args, &[]);
        std::thread::sleep(Duration::from_millis(80 + rng.below(400)));
        c.kill().unwrap(); // SIGKILL
        let _ = c.wait();
        kills += 1;
    }
    // A last run drains the inbox, then stops.
    let c = spawn(&dir, &args, &[]);
    let pending = |ls: &[Line]| {
        let accepted: BTreeSet<String> = ls
            .iter()
            .filter(|l| l.kind == "stimulus.accepted")
            .map(|l| l.get("item")["id"].as_str().unwrap_or("").to_string())
            .collect();
        let disposed: BTreeSet<String> = ls
            .iter()
            .filter(|l| l.kind == "stimulus.disposed")
            .map(|l| l.str("id").to_string())
            .collect();
        accepted.len() >= sent && accepted.is_subset(&disposed)
    };
    wait_for(&dir, 300, "the inbox to drain", pending);
    let (_, code) = stop(c);
    assert_eq!(code, Some(0));
    let ls = lines(&dir);
    let replayed = State::replay_hashes(&ls);
    assert_gate(&gates::g_i(&ls, kills, &replayed));
    assert_gate(&gates::g_k(&ls));
}
