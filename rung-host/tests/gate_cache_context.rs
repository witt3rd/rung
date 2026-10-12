//! G-j (bounded context) and G-l (cache discipline) over one 10,000-turn
//! run; G-k over the same record; G-l's two-process stability.

mod common;

use common::*;
use rung_host::clock::DAY;
use rung_host::gates;
use rung_host::sim;

#[test]
fn ten_thousand_turns_stay_bounded_and_cache_clean() {
    sim::test_timeout(1800);
    let mut sc = scenario("gate-jl", 41);
    sc.max_turns = Some(gates::G_J_TURNS);
    sc.world = busy_world(41, 2 * DAY);
    sc.faults = cache_and_ladder_faults();
    sc.config.epoch_budget_tokens = 24_000;
    // The agent finishes its todos and projects (as a real one does): with the register lists
    // shown whole, one that only ever added would outgrow any budget.
    sc.mock.p_todo_done = 0.2;
    sc.mock.settles_projects = true;
    sc.mock.copy_every = 40;
    sc.mock.copy_run = 4;
    // The baseline provider re-reads its whole store on every call, which
    // is quadratic over 10,000 turns; memory is exercised by G-c's run.
    sc.memory = false;
    let out = sim::run(sc);
    assert_gate(&gates::g_j(&out.lines));
    assert_gate(&gates::g_l(&out.lines, &out.captured));
    assert_gate(&gates::g_k(&out.lines));
}

/// The case the tuned run above leaves out: an agent that never closes a todo or a project.
/// Its registers are shown whole, so its header grows with its content, by design: no cap
/// stops that. What stays true is the pack's own bound: no prompt passes the epoch budget
/// (the host rolls the epoch over as the header grows), and the only gate checks that give
/// way are the two that assume a header of bounded size (the 85% ceiling at turn start and
/// the record's linear growth).
#[test]
fn an_agent_that_never_closes_its_work_grows_the_header_not_past_the_budget() {
    sim::test_timeout(1800);
    let mut sc = scenario("gate-j-open", 41);
    sc.max_turns = Some(gates::G_J_TURNS);
    sc.world = busy_world(41, 2 * DAY);
    sc.faults = cache_and_ladder_faults();
    sc.config.epoch_budget_tokens = 24_000;
    sc.mock.copy_every = 40;
    sc.mock.copy_run = 4;
    sc.memory = false;
    let out = sim::run(sc);
    let g = gates::g_j(&out.lines);
    eprintln!("GATE {}", g.summary());
    let content_driven = [
        "a turn started above 85% of the budget",
        "the record does not grow linearly",
    ];
    let other: Vec<_> = g
        .failures
        .iter()
        .filter(|f| !content_driven.contains(&f.as_str()))
        .collect();
    assert!(other.is_empty(), "{other:?}\n{}", g.summary());
    // and the growth is real: the header outgrows what a closing agent's stays at
    let rollovers = out
        .lines
        .iter()
        .filter(|l| l.kind == "epoch.rollover")
        .count();
    assert!(
        rollovers > 458,
        "the header did not grow: {rollovers} rollovers"
    );
}

#[test]
fn canonical_bytes_are_stable_across_two_processes() {
    sim::test_timeout(300);
    let bin = env!("CARGO_BIN_EXE_rung-host");
    let run = |name: &str| {
        let _guard = sim::temp_dir_guard(name);
        let dir = _guard.path().to_path_buf();
        let out = std::process::Command::new(bin)
            .args([
                "canon",
                "--state",
                dir.to_str().unwrap(),
                "--seed",
                "5",
                "--turns",
                "300",
                "--no-memory",
            ])
            .output()
            .expect("run rung-host");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    assert_gate(&gates::g_l_stable(&run("canon-a"), &run("canon-b")));
}
