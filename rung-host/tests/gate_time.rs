//! G-a (no rest), G-b (responsiveness), G-d (schedule), G-e
//! (expectations); G-k over each run.

mod common;

use std::sync::Arc;

use common::*;
use rung_host::calendar::{Entry, Missed, Origin, When};
use rung_host::clock::{HOUR, MINUTE, Millis, SimClock};
use rung_host::desk::{DeskMode, Scripted, Step};
use rung_host::gates;
use rung_host::sim::{self, DeskSpec, SIM_START, WorldConfig};

/// G-a measures the host's own wall-clock work per boundary; run these tests one at a time so
/// sibling simulations do not steal the CPU and inflate it.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
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
    let _serial = serial();
    sim::test_timeout(1800);
    let mut sc = scenario("gate-a", 3);
    sc.until = Some(SIM_START + gates::G_A_RUN_MS + MINUTE);
    sc.desk = scripted(Step::Seeded(3));
    let out = sim::run(sc);
    assert_gate(&gates::g_a(&out.lines));
    assert_gate(&gates::g_k(&out.lines));
}

#[test]
fn owner_stimuli_under_load_are_admitted_at_the_next_boundary() {
    let _serial = serial();
    sim::test_timeout(1800);
    let mut sc = scenario("gate-b", 5);
    sc.until = Some(SIM_START + 3 * HOUR);
    sc.world = WorldConfig {
        owner_per_hour: 30.0,
        peer_per_hour: 40.0,
        ..busy_world(5, 3 * HOUR)
    };
    // Long work is rare (a few events per run): a refused call still spends the tool deadline,
    // and an owner item that lands in such a turn waits it out.
    sc.mock.p_long_work = 0.001;
    sc.mock.p_long_work_responding = 0.001;
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

#[test]
fn due_items_fire_at_the_first_boundary_and_missed_ones_once() {
    let _serial = serial();
    sim::test_timeout(1800);
    let _guard = sim::temp_dir_guard("gate-d");
    let dir = _guard.path().to_path_buf();
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
    // The decider never wants anything shown: firm items must still be.
    let adversarial = || {
        scripted(Step::Uniform {
            p: 0.0,
            pick: "at_break".into(),
        })
    };
    let mut sc = rung_host::sim::Scenario::new(&dir, 7);
    sc.until = Some(gap_from);
    sc.seed_calendar = cal;
    sc.desk = adversarial();
    let first = sim::run(sc);
    assert!(first.lines.iter().any(|l| l.kind == "calendar.fired"));
    // The host is down from `gap_from` to `gap_to`, then wakes.
    let mut sc = rung_host::sim::Scenario::new(&dir, 7);
    sc.clock = Some(Arc::new(SimClock::new(gap_to)));
    sc.until = Some(gap_to + HOUR);
    sc.desk = adversarial();
    let out = sim::run(sc);
    assert_gate(&gates::g_d(&out.lines));
    assert_gate(&gates::g_d_downtime(
        &out.lines,
        &["down-late", "down-skip", "every-20m"],
    ));
    assert_gate(&gates::g_k(&out.lines));
}

#[test]
fn only_the_host_settles_expectations_and_calibration_recomputes() {
    let _serial = serial();
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
    let out = sim::run(sc);
    assert_gate(&gates::g_e(&out.lines));
    assert_gate(&gates::g_k(&out.lines));
}
