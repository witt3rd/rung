//! #159: how owner admission latency depends on the share of long work. A measurement, not a
//! gate: it is `#[ignore]`d (several G-b-sized runs) and run by hand:
//!
//! ```text
//! cargo test -p rung-host --test measure_admission -- --ignored --nocapture
//! ```
//!
//! Each run is G-b's scenario with only the long-work share changed. It prints, per share,
//! the measured admission p95, the turn p95 G-b compares it with, and the p95 wait an arrival
//! uniform in time has for the next boundary given the run's cycle lengths
//! ([`gates::admission_bias`]). It asserts the host-side property under every share (each
//! owner stimulus is admitted at the first boundary after it arrived) and that G-b holds at
//! every share with long work, now that a long call is cut when an owner waits.

mod common;

use common::*;
use rung_host::clock::HOUR;
use rung_host::desk::{DeskMode, Scripted, Step};
use rung_host::gates;
use rung_host::sim::{self, DeskSpec, SIM_START, WorldConfig};

const SHARES: &[f64] = &[0.0, 0.001, 0.002, 0.005, 0.01];
/// G-b's seed first, then four more: one seed is a lottery on where long calls land.
const SEEDS: &[u64] = &[5, 6, 7, 8, 9];

#[test]
#[ignore = "a measurement: several G-b-sized runs"]
fn admission_latency_by_long_work_share() {
    sim::test_timeout(3600);
    let rows: Vec<_> = std::thread::scope(|s| {
        let hs: Vec<_> = SHARES
            .iter()
            .flat_map(|&share| SEEDS.iter().map(move |&seed| (share, seed)))
            .map(|(share, seed)| {
                s.spawn(move || {
                    let mut sc = scenario(&format!("measure-admission-{share}-{seed}"), seed);
                    sc.until = Some(SIM_START + 3 * HOUR);
                    sc.world = WorldConfig {
                        owner_per_hour: 30.0,
                        peer_per_hour: 40.0,
                        ..busy_world(seed, 3 * HOUR)
                    };
                    sc.mock.p_long_work = share;
                    sc.mock.p_long_work_responding = share;
                    sc.desk = DeskSpec::Decider {
                        decider: std::sync::Arc::new(Scripted::always(Step::Seeded(seed))),
                        backend: "scripted".into(),
                        mode: DeskMode::Decide,
                    };
                    let out = sim::run(sc);
                    let g = gates::g_b(&out.lines);
                    let b = gates::admission_bias(&out.lines);
                    (share, seed, g, b)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    eprintln!(
        "MEASURE share | seed | owner | admission_p95_ms | turn_p95_ms | residual_p95_ms | long_time_share | long_refused | cut_for_owner | owner_in_long_cycles | after_next_boundary | G-b"
    );
    for (share, seed, g, b) in &rows {
        let m = &g.measured;
        eprintln!(
            "MEASURE {share} | {seed} | {} | {} | {} | {} | {:.4} | {} | {} | {} | {} | {}",
            m["owner_stimuli"],
            m["admission_p95_ms"],
            m["turn_p95_ms"],
            b.residual_p95_ms,
            b.long_time_share,
            m["long_work_refused"],
            m["long_work_cut_for_owner"],
            b.owner_in_long_cycles,
            b.after_next_boundary,
            if g.pass { "PASS" } else { "FAIL" },
        );
    }
    for (share, seed, g, _) in &rows {
        // With long work cut for a waiting owner, G-b holds at every share (it fails at 0
        // only for want of long work to check).
        assert!(
            *share == 0.0 || g.pass,
            "share {share} seed {seed}: {:?}",
            g.failures
        );
    }
    for (share, seed, _, b) in &rows {
        assert_eq!(
            b.after_next_boundary, 0,
            "share {share} seed {seed}: an owner stimulus waited past the first boundary after it arrived"
        );
    }
}
