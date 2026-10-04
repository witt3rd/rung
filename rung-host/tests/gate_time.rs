//! G-a (no rest), G-b (responsiveness), G-d (schedule), G-e
//! (expectations); G-k over each run.

mod common;

use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;

use common::*;
use rung_host::calendar::{Entry, Missed, Origin, When};
use rung_host::clock::{HOUR, MINUTE, Millis, SimClock};
use rung_host::desk::{DeskMode, Scripted, Step};
use rung_host::gates;
use rung_host::sim::{self, DeskSpec, SIM_START, WorldConfig};

/// G-a measures the host's own wall-clock work per boundary, so it runs alone: sibling
/// simulations would steal the CPU and inflate it. The other gates here read only the
/// simulated clock, so they share the machine with each other. Their one wall-clock input is
/// the desk's 1.9 s ask deadline, which a scripted decider meets in microseconds; when it is
/// missed the boundary falls back to the rule (`g_d_holds_when_every_ask_times_out`).
static WALL: RwLock<()> = RwLock::new(());

/// Exclusive: a gate that measures wall time.
fn alone() -> RwLockWriteGuard<'static, ()> {
    WALL.write().unwrap_or_else(|e| e.into_inner())
}

/// Shared: a gate on the simulated clock only.
fn shared() -> RwLockReadGuard<'static, ()> {
    WALL.read().unwrap_or_else(|e| e.into_inner())
}

fn scripted(step: Step) -> DeskSpec {
    DeskSpec::Decider {
        decider: Arc::new(Scripted::always(step)),
        backend: "scripted".into(),
        mode: DeskMode::Decide,
    }
}

#[test]
fn thirty_quiet_minutes_have_no_rest() {
    let _wall = alone();
    sim::test_timeout(1800);
    let mut sc = scenario("gate-a", 3);
    sc.until = Some(SIM_START + gates::G_A_RUN_MS + MINUTE);
    sc.desk = scripted(Step::Seeded(3));
    let dir = sc.dir.clone();
    let out = sim::run(sc);
    assert_gate_in(&dir, &out.lines, &gates::g_a(&out.lines));
    assert_gate(&gates::g_k(&out.lines));
}

#[test]
fn owner_stimuli_under_load_are_admitted_at_the_next_boundary() {
    let _sim = shared();
    sim::test_timeout(1800);
    let mut sc = scenario("gate-b", 5);
    sc.until = Some(SIM_START + 3 * HOUR);
    sc.world = WorldConfig {
        owner_per_hour: 30.0,
        peer_per_hour: 40.0,
        ..busy_world(5, 3 * HOUR)
    };
    // 1% long work (about 25 long calls a run). A long call that runs past the cut-off is cut
    // as soon as an owner waits (#159); before that the owner waited out the 30 s tool
    // deadline and this scenario failed (admission p95 15.9 s against a turn p95 of 1.7 s).
    sc.mock.p_long_work = 0.01;
    sc.mock.p_long_work_responding = 0.01;
    sc.desk = scripted(Step::Seeded(5));
    let dir = sc.dir.clone();
    let out = sim::run(sc);
    assert_gate_in(&dir, &out.lines, &gates::g_b(&out.lines));
    assert_gate(&gates::g_k(&out.lines));
}

fn entry(id: &str, at: Millis, firm: bool, missed: Missed) -> Entry {
    Entry {
        id: id.into(),
        when: When::At(at),
        origin: Origin::Owner,
        text: format!("calendar item {id}"),
        firm,
        missed,
    }
}

/// G-d's two runs (a calendar, a 2 h downtime, a wake) over `dir`. `desk` builds each run's
/// desk; `timeout` replaces the desk's ask deadline. Returns the waking run's lines.
fn g_d_runs(
    dir: &std::path::Path,
    desk: impl Fn() -> DeskSpec,
    timeout: Option<Duration>,
    memory: bool,
) -> Vec<rung_host::record::Line> {
    let gap_from = SIM_START + 2 * HOUR;
    let gap_to = SIM_START + 4 * HOUR;
    let mut cal = Vec::new();
    // Before the gap: every 7 minutes, every third one firm.
    for k in 1..16 {
        cal.push(entry(
            &format!("c{k}"),
            SIM_START + k * 7 * MINUTE + 13,
            k % 3 == 0,
            Missed::OnceLate,
        ));
    }
    // Inside the gap: missed while down.
    cal.push(entry(
        "down-late",
        gap_from + 30 * MINUTE,
        true,
        Missed::OnceLate,
    ));
    cal.push(entry(
        "down-skip",
        gap_from + 40 * MINUTE,
        false,
        Missed::Skip,
    ));
    cal.push(Entry {
        id: "every-20m".into(),
        when: When::Every {
            start: SIM_START + 5 * MINUTE,
            period: 20 * MINUTE,
        },
        origin: Origin::Routine,
        text: "a standing routine".into(),
        firm: false,
        missed: Missed::OnceLate,
    });
    // After the gap.
    for k in 1..6 {
        cal.push(entry(
            &format!("after{k}"),
            gap_to + k * 9 * MINUTE,
            k % 2 == 0,
            Missed::OnceLate,
        ));
    }
    let mut sc = rung_host::sim::Scenario::new(dir, 7);
    sc.until = Some(gap_from);
    sc.seed_calendar = cal;
    sc.desk = desk();
    sc.desk_timeout = timeout;
    sc.memory = memory;
    let first = sim::run(sc);
    assert!(first.lines.iter().any(|l| l.kind == "calendar.fired"));
    // The host is down from `gap_from` to `gap_to`, then wakes.
    let mut sc = rung_host::sim::Scenario::new(dir, 7);
    sc.clock = Some(Arc::new(SimClock::new(gap_to)));
    sc.until = Some(gap_to + HOUR);
    sc.desk = desk();
    sc.desk_timeout = timeout;
    sc.memory = memory;
    sim::run(sc).lines
}

fn assert_g_d(dir: &std::path::Path, lines: &[rung_host::record::Line]) {
    assert_gate_in(dir, lines, &gates::g_d(lines));
    assert_gate_in(
        dir,
        lines,
        &gates::g_d_downtime(lines, &["down-late", "down-skip", "every-20m"]),
    );
    assert_gate(&gates::g_k(lines));
}

/// The decider never wants anything shown: firm items must still be.
fn adversarial() -> DeskSpec {
    scripted(Step::Uniform {
        p: 0.0,
        pick: "at_break".into(),
    })
}

#[test]
fn due_items_fire_at_the_first_boundary_and_missed_ones_once() {
    let _sim = shared();
    sim::test_timeout(1800);
    let guard = sim::temp_dir_guard("gate-d");
    let lines = g_d_runs(guard.path(), adversarial, None, true);
    assert_g_d(guard.path(), &lines);
}

/// The worst a loaded machine can do to a simulated run is make every desk ask miss its
/// wall-clock deadline (its one wall-clock input). Then every boundary falls back to the
/// rule, and G-d holds all the same: the gate's verdict does not depend on machine load.
#[test]
fn g_d_holds_when_every_ask_times_out() {
    let _sim = shared();
    sim::test_timeout(1800);
    let guard = sim::temp_dir_guard("gate-d-timeout");
    let stalled = || {
        scripted(Step::Delay(
            50,
            Box::new(Step::Uniform {
                p: 0.0,
                pick: "at_break".into(),
            }),
        ))
    };
    // Memory is not what this run is about, and off it runs in seconds.
    let lines = g_d_runs(guard.path(), stalled, Some(Duration::from_millis(1)), false);
    let asks: Vec<&str> = lines
        .iter()
        .filter(|l| l.kind == "desk.ask")
        .map(|l| l.str("outcome"))
        .collect();
    assert!(!asks.is_empty(), "no desk asks");
    assert!(
        asks.iter()
            .all(|o| *o == "timeout" || *o == "nothing_to_ask"),
        "an ask beat a 1 ms deadline: {:?}",
        asks.iter().filter(|o| **o != "timeout").collect::<Vec<_>>()
    );
    assert_g_d(guard.path(), &lines);
}

#[test]
fn only_the_host_settles_expectations_and_calibration_recomputes() {
    let _sim = shared();
    sim::test_timeout(1800);
    let mut sc = scenario("gate-e", 9);
    sc.until = Some(SIM_START + 2 * HOUR);
    sc.world = WorldConfig {
        facts_per_hour: 6.0,
        fact_keys: vec!["build".into()],
        ..busy_world(9, 2 * HOUR)
    };
    sc.mock.p_expect = 0.3;
    // Hours of turns: memory's store would be re-read on every call.
    sc.memory = false;
    let dir = sc.dir.clone();
    let out = sim::run(sc);
    assert_gate_in(&dir, &out.lines, &gates::g_e(&out.lines));
    assert_gate(&gates::g_k(&out.lines));
}
