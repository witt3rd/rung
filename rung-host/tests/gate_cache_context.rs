//! G-j (bounded context) and G-l (cache discipline) over one 10,000-turn
//! run; G-k over the same record; G-l's two-process stability.

mod common;

use common::*;
use rung_host::clock::DAY;
use rung_host::gates;
use rung_host::sim;

#[test]
fn ten_thousand_turns_stay_bounded_and_cache_clean() {
    sim::test_timeout(900);
    let mut sc = scenario("gate-jl", 41);
    sc.max_turns = Some(gates::G_J_TURNS);
    sc.world = busy_world(41, 2 * DAY);
    sc.faults = cache_and_ladder_faults();
    sc.config.epoch_budget_tokens = 24_000;
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

#[test]
fn canonical_bytes_are_stable_across_two_processes() {
    sim::test_timeout(300);
    let bin = env!("CARGO_BIN_EXE_rung-host");
    let run = |name: &str| {
        let dir = sim::temp_dir(name);
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
