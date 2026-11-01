//! Idle mode (idle time above 0): the host rests between wakes on a fake
//! clock and a fake queue. Claims: `docs/rung-host-idle-props.md`. The
//! idle-time-0 gate (G-a, `gate_time.rs`) is unchanged.

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use rung_host::calendar::{Entry, Missed, Origin, When};
use rung_host::clock::{HOUR, MINUTE, Millis, SECOND};
use rung_host::inbox::{Item, MemorySource, Role, Source};
use rung_host::record::{Line, Record};
use rung_host::sim::{self, Fault, FaultKind, SIM_START};
use rung_host::state::State;
use rung_host::stop::{StopAuthority, Why};

const IDLE: Millis = 30 * MINUTE;

fn idle_scenario(name: &str, until: Millis) -> sim::Scenario {
    let mut sc = scenario(name, 11);
    sc.config.idle_time_ms = IDLE;
    sc.until = Some(SIM_START + until);
    sc
}

/// Index pairs `(idle, idle.ended)` of every rest, in order.
fn rests(lines: &[Line]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut open: Option<usize> = None;
    for (i, l) in lines.iter().enumerate() {
        match l.kind.as_str() {
            "idle" => {
                assert!(open.is_none(), "an idle line inside a rest: #{}", l.seq);
                open = Some(i);
            }
            "idle.ended" => out.push((open.take().expect("idle.ended without idle"), i)),
            _ => {}
        }
    }
    assert!(open.is_none(), "a rest was never ended");
    out
}

/// The turn kinds of the `turn.ended` lines in `lines`.
fn turn_kinds(lines: &[Line]) -> Vec<String> {
    lines
        .iter()
        .filter(|l| l.kind == "turn.ended")
        .map(|l| l.str("turn_kind").to_string())
        .collect()
}

/// For each turn in `lines`, in order: whether it is a free-time pass (a
/// free turn that took no item: nothing admitted, not even as a digest).
fn passes(lines: &[Line]) -> Vec<bool> {
    let taken: BTreeSet<u64> = lines
        .iter()
        .filter(|l| l.kind == "stimulus.admitted")
        .map(|l| l.u64("turn"))
        .collect();
    lines
        .iter()
        .filter(|l| l.kind == "turn.ended")
        .map(|l| l.str("turn_kind") == "free" && !taken.contains(&l.u64("turn")))
        .collect()
}

/// Exactly one free-time pass, and it is the last turn.
fn one_pass_last(lines: &[Line]) {
    let p = passes(lines);
    assert_eq!(
        p.iter().filter(|x| **x).count(),
        1,
        "{:?}",
        turn_kinds(lines)
    );
    assert_eq!(p.last(), Some(&true), "{:?}", turn_kinds(lines));
}

fn calendar_entry(id: &str, at: Millis) -> Entry {
    Entry {
        id: id.into(),
        when: When::At(at),
        origin: Origin::Owner,
        text: format!("calendar item {id}"),
        firm: true,
        missed: Missed::OnceLate,
    }
}

fn owner(id: &str, at: Millis) -> Item {
    Item::message(id, Role::Owner, "owner", at, &format!("owner asks {id}?"))
}

/// A queue that raises the stop authority when its time comes (a signal
/// or a stop file arriving mid-rest).
struct StopAt {
    at: Millis,
    stop: Arc<StopAuthority>,
    fired: bool,
}

impl Source for StopAt {
    fn poll(&mut self, now: Millis, _seen: &BTreeSet<String>) -> Vec<Item> {
        if !self.fired && now >= self.at {
            self.fired = true;
            self.stop.request(Why::Stopped { by: "test".into() });
        }
        Vec::new()
    }

    fn next_at(&self) -> Option<Millis> {
        (!self.fired).then_some(self.at)
    }
}

// ─── an-idle-host-really-rests, a-scheduled-wake-is-one-free-time-pass ───────

#[test]
fn an_idle_host_rests_and_a_scheduled_wake_is_one_free_pass() {
    sim::test_timeout(300);
    let sc = idle_scenario("idle-rest", 3 * HOUR);
    let out = sim::run(sc);
    let ls = &out.lines;
    let rs = rests(ls);
    assert!(rs.len() >= 5, "rests: {}", rs.len());
    let mut saw_full = false;
    for (a, b) in &rs {
        let (idle, ended) = (&ls[*a], &ls[*b]);
        // within a rest: nothing but the rest's own wake conditions
        for l in &ls[*a + 1..*b] {
            assert!(
                !matches!(
                    l.kind.as_str(),
                    "boundary" | "turn.started" | "turn.ended" | "llm.call" | "degraded"
                ),
                "{} inside a rest at #{}",
                l.kind,
                l.seq
            );
        }
        // the scheduled wake is at most idle time away, and no later than
        // the next due calendar item or open expectation, read from the fold
        let st = State::replay(&ls[..*a]);
        let due = st
            .calendar
            .next_due()
            .into_iter()
            .chain(
                st.registers
                    .expectations
                    .values()
                    .filter(|e| e.state == rung_host::registers::ExpState::Open)
                    .map(|e| e.due + 1),
            )
            .chain([SIM_START + 3 * HOUR]) // the run limit ends a rest too
            .filter(|d| *d > idle.at)
            .fold(idle.at + IDLE, Millis::min);
        assert_eq!(idle.i64("until"), due);
        assert!(idle.i64("until") - idle.at <= IDLE);
        saw_full |= idle.i64("until") - idle.at == IDLE;
        assert_eq!(idle.i64("idle_time_ms"), IDLE);
        assert_eq!(ended.str("cause"), "scheduled");
        assert_eq!(
            ended.at,
            idle.i64("until"),
            "{}",
            record_tail(&ls[*a..=*b], 6)
        );
        assert_eq!(ended.i64("waited_ms"), idle.i64("until") - idle.at);
    }
    assert!(saw_full, "at least one rest runs the whole idle time");
    // between two rests: one free-time pass at the end of what the wake
    // brought (an item the agent scheduled for itself is answered first)
    for w in rs.windows(2) {
        one_pass_last(&ls[w[0].1 + 1..w[1].0]);
    }
    // the first pass is at the start: the start counts as a wake
    one_pass_last(&ls[..rs[0].0]);
    // a rest is not degraded
    assert!(!ls.iter().any(|l| l.kind == "degraded"));
    // the fold of the record equals the live state, idle included
    assert_eq!(out.host.core.state().hash(), State::replay(ls).hash());
    assert_gate(&rung_host::gates::g_k(ls));
}

// ─── the-scheduled-wake-is-within-idle-time ──────────────────────────────────

#[test]
fn a_due_calendar_item_ends_a_rest_and_is_answered_before_the_pass() {
    sim::test_timeout(300);
    let due = SIM_START + 10 * MINUTE;
    let mut sc = idle_scenario("idle-calendar", HOUR);
    sc.seed_calendar = vec![calendar_entry("tea", due)];
    let out = sim::run(sc);
    let ls = &out.lines;
    let rs = rests(ls);
    let (idle, ended) = (&ls[rs[0].0], &ls[rs[0].1]);
    assert_eq!(
        idle.i64("until"),
        due,
        "the wake is no later than the next due item"
    );
    assert_eq!(ended.at, due);
    let fired = ls
        .iter()
        .find(|l| l.kind == "calendar.fired")
        .expect("the item fired");
    assert_eq!(
        fired.at, due,
        "it fires at the first boundary after it is due"
    );
    assert!(fired.seq > ended.seq);
    // answered (responding), then one free pass, then rest
    let after = &ls[rs[0].1 + 1..rs[1].0];
    one_pass_last(after);
    assert!(turn_kinds(after).iter().any(|k| k == "responding"));
}

// ─── an-arrival-wakes-at-once, an-arrival-drains-then-one-free-time-pass ─────

#[test]
fn an_arrival_wakes_at_once_drains_the_queue_then_one_free_pass() {
    sim::test_timeout(300);
    let arrive = SIM_START + 40 * MINUTE;
    let mut q = MemorySource::default();
    // the second arrives while the first is being answered
    q.push(owner("o1", arrive));
    q.push(owner("o2", arrive + SECOND));
    let mut sc = idle_scenario("idle-arrival", 3 * HOUR);
    sc.sources.push(Box::new(q));
    let out = sim::run(sc);
    let ls = &out.lines;
    let rs = rests(ls);
    // the rest that holds the arrival ends at it, before its scheduled wake
    let (a, b) = *rs
        .iter()
        .find(|(a, b)| ls[*a].at < arrive && ls[*b].at >= arrive)
        .expect("a rest spans the arrival");
    assert_eq!(
        ls[b].at, arrive,
        "woken at the arrival, not at the schedule"
    );
    assert!(ls[b].at < ls[a].i64("until"));
    assert_eq!(ls[b].str("cause"), "arrival");
    // every waiting item is answered, then exactly one free pass, then rest
    let next = rs.iter().find(|(x, _)| *x > b).expect("rests again");
    one_pass_last(&ls[b + 1..next.0]);
    let kinds = turn_kinds(&ls[b + 1..next.0]);
    assert!(kinds.len() >= 2 && kinds[0] == "responding", "{kinds:?}");
    let disposed: Vec<&str> = ls[b + 1..next.0]
        .iter()
        .filter(|l| l.kind == "stimulus.disposed")
        .map(|l| l.str("id"))
        .collect();
    assert!(
        disposed.contains(&"o1") && disposed.contains(&"o2"),
        "{disposed:?}"
    );
    // the arrival is on the record before anything else (before the next boundary)
    let accepted = ls
        .iter()
        .position(|l| l.kind == "stimulus.accepted" && l.get("item")["id"] == "o1")
        .expect("accepted");
    assert!(accepted > a && accepted < b);
    // the new wake is idle time after the pass, not the old schedule
    let after = &ls[next.0];
    assert_eq!(after.i64("until") - after.at, IDLE);
}

// ─── a-rest-is-not-degraded ──────────────────────────────────────────────────

#[test]
fn a_world_imposed_wait_is_degraded_and_never_a_rest() {
    sim::test_timeout(300);
    let mut q = MemorySource::default();
    q.push(owner("o1", SIM_START + 40 * MINUTE));
    let mut sc = idle_scenario("idle-degraded", 3 * HOUR);
    sc.sources.push(Box::new(q));
    // the provider is down while the owner's item waits
    sc.faults = vec![Fault {
        from: SIM_START + 35 * MINUTE,
        to: SIM_START + 50 * MINUTE,
        kind: FaultKind::Outage,
        model: None,
    }];
    let out = sim::run(sc);
    let ls = &out.lines;
    let waits: Vec<(usize, usize)> = {
        let mut open = None;
        let mut v = Vec::new();
        for (i, l) in ls.iter().enumerate() {
            match l.kind.as_str() {
                "degraded" => open = Some(i),
                "degraded.ended" => v.push((open.take().expect("open wait"), i)),
                _ => {}
            }
        }
        v
    };
    assert!(!waits.is_empty(), "the outage is a degraded wait");
    for (a, b) in &waits {
        assert!(
            !ls[*a..=*b].iter().any(|l| l.kind.starts_with("idle")),
            "a degraded wait is not a rest"
        );
    }
    // and a rest holds no degraded line (checked on every rest)
    for (a, b) in rests(ls) {
        assert!(!ls[a..=b].iter().any(|l| l.kind == "degraded"));
    }
    // the item is answered once the provider is back
    assert!(ls.iter().any(|l| l.kind == "stimulus.disposed"
        && l.str("id") == "o1"
        && l.at >= SIM_START + 50 * MINUTE));
}

// ─── the-stop-authority-interrupts-a-rest ────────────────────────────────────

#[test]
fn a_stop_mid_rest_halts_at_once() {
    sim::test_timeout(300);
    let stop_at = SIM_START + 20 * MINUTE;
    let stop = Arc::new(StopAuthority::default());
    let mut sc = idle_scenario("idle-stop", 10 * HOUR);
    sc.stop = Some(stop.clone());
    sc.sources.push(Box::new(StopAt {
        at: stop_at,
        stop,
        fired: false,
    }));
    let out = sim::run(sc);
    let ls = &out.lines;
    let halted = ls.last().unwrap();
    assert_eq!(halted.kind, "halted");
    assert_eq!(
        halted.at, stop_at,
        "halted at the stop, not at the scheduled wake"
    );
    let (_, b) = *rests(ls).last().unwrap();
    assert_eq!(ls[b].str("cause"), "stop");
    assert!(
        !ls[b + 1..]
            .iter()
            .any(|l| matches!(l.kind.as_str(), "boundary" | "turn.started")),
        "no boundary or turn after the stop"
    );
    assert!(matches!(out.why, Why::Stopped { .. }));
}

// ─── idle-time-zero-never-rests (beside G-a) ─────────────────────────────────

#[test]
fn idle_time_zero_writes_no_idle_line() {
    sim::test_timeout(300);
    let mut sc = scenario("idle-zero", 11);
    sc.max_turns = Some(20);
    assert_eq!(sc.config.idle_time_ms, 0);
    let out = sim::run(sc);
    assert_eq!(turn_kinds(&out.lines).len(), 20);
    assert!(!out.lines.iter().any(|l| l.kind.starts_with("idle")));
}

// ─── the state's view of a rest (what a reader folds) ────────────────────────

#[test]
fn the_fold_is_idle_from_the_idle_line_until_it_ends() {
    sim::test_timeout(300);
    let out = sim::run(idle_scenario("idle-fold", HOUR));
    let ls = &out.lines;
    let (a, b) = rests(ls)[0];
    assert_eq!(State::replay(&ls[..a]).idle_until, None);
    assert_eq!(
        State::replay(&ls[..=a]).idle_until,
        Some(ls[a].i64("until"))
    );
    assert_eq!(State::replay(&ls[..=b]).idle_until, None);
}

// ─── the binary: the 30-minute default, a stop reaches a rest ────────────────

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");

/// A spawned host, killed when the test ends, however it ends.
struct Kid(Child);

impl Drop for Kid {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn(root: &Path, extra: &str, args: &[&str]) -> Kid {
    let cfg = root.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "state: {d}/state\nstop_file: {d}/STOP\nengine:\n  kind: mock\nmemory: false\n{extra}",
            d = root.display()
        ),
    )
    .unwrap();
    Kid(Command::new(BIN)
        .args(["run", "--config"])
        .arg(&cfg)
        .args(args)
        .env("RUNG_HOME", root.join("rung-home"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rung-host"))
}

fn lines(root: &Path) -> Vec<Line> {
    Record::read_dir(root.join("state").join("record")).unwrap_or_default()
}

fn wait_for(root: &Path, what: &str, f: impl Fn(&[Line]) -> bool) {
    let t = Instant::now();
    let limit = Duration::from_secs_f64(60.0 * load_factor());
    while t.elapsed() < limit {
        if f(&lines(root)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "timed out waiting for {what}\n{}",
        record_tail(&lines(root), 30)
    );
}

#[test]
fn a_config_without_idle_time_rests_for_thirty_minutes_and_a_stop_file_ends_it() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("idle-default");
    let root = guard.path().to_path_buf();
    let mut child = spawn(&root, "", &[]);
    wait_for(&root, "the first rest", |ls| {
        ls.iter().any(|l| l.kind == "idle")
    });
    let ls = lines(&root);
    let start = ls.iter().find(|l| l.kind == "host.start").unwrap();
    assert_eq!(start.get("config")["idle_time_ms"], 30 * 60 * 1000);
    let idle = ls.iter().find(|l| l.kind == "idle").unwrap();
    assert_eq!(idle.i64("until") - idle.at, IDLE);
    // exactly one free-time pass came before the first rest
    let before = &ls[..ls.iter().position(|l| l.kind == "idle").unwrap()];
    one_pass_last(before);
    // the stop file interrupts the rest at once
    std::fs::write(root.join("STOP"), "").unwrap();
    wait_for(&root, "the halt", |ls| {
        ls.last().is_some_and(|l| l.kind == "halted")
    });
    let status = child.0.wait().unwrap();
    assert!(status.success(), "{status:?}");
    let ls = lines(&root);
    let ended = ls.iter().rev().find(|l| l.kind == "idle.ended").unwrap();
    assert_eq!(ended.str("cause"), "stop");
    assert!(ended.i64("waited_ms") < IDLE / 10);
}

#[test]
fn sigterm_ends_a_rest_at_once() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("idle-sigterm");
    let root = guard.path().to_path_buf();
    let mut child = spawn(&root, "", &[]);
    wait_for(&root, "the first rest", |ls| {
        ls.iter().any(|l| l.kind == "idle")
    });
    // SAFETY: signalling our own child.
    unsafe {
        libc::kill(child.0.id() as libc::pid_t, libc::SIGTERM);
    }
    wait_for(&root, "the halt", |ls| {
        ls.last().is_some_and(|l| l.kind == "halted")
    });
    let status = child.0.wait().unwrap();
    assert!(status.success(), "{status:?}");
    let ls = lines(&root);
    let ended = ls.iter().rev().find(|l| l.kind == "idle.ended").unwrap();
    assert_eq!(ended.str("cause"), "stop");
    assert!(ended.i64("waited_ms") < IDLE / 10);
}

#[test]
fn idle_time_zero_in_a_config_is_continuous() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("idle-zero-config");
    let root = guard.path().to_path_buf();
    let mut child = spawn(&root, "idle_time_s: 0\n", &["--turns", "5"]);
    let status = child.0.wait().unwrap();
    assert!(status.success(), "{status:?}");
    let ls = lines(&root);
    assert_eq!(
        ls.iter()
            .find(|l| l.kind == "host.start")
            .unwrap()
            .get("config")["idle_time_ms"],
        0
    );
    assert!(turn_kinds(&ls).len() >= 5);
    assert!(!ls.iter().any(|l| l.kind.starts_with("idle")));
}

#[test]
fn a_negative_idle_time_is_refused_naming_the_key() {
    sim::test_timeout(60);
    let guard = sim::temp_dir_guard("idle-negative");
    let root = guard.path().to_path_buf();
    let mut child = spawn(&root, "idle_time_s: -1\n", &[]);
    let status = child.0.wait().unwrap();
    let mut err = String::new();
    std::io::Read::read_to_string(&mut child.0.stderr.take().unwrap(), &mut err).unwrap();
    assert_eq!(status.code(), Some(2));
    assert!(err.contains("idle_time_s"), "{err}");
    assert!(
        !root.join("state").join("record").exists(),
        "state untouched: {err}"
    );
}
