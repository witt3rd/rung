//! G-g: provider failure and quota, never death. G-k over the run.

mod common;

use common::*;
use rung_host::clock::{DAY, HOUR, MINUTE};
use rung_host::gates;
use rung_host::sim::{self, Fault, FaultKind, SIM_START, WorldConfig};

fn at(h: i64, m: i64) -> i64 {
    SIM_START + h * HOUR + m * MINUTE
}

#[test]
fn injected_faults_degrade_and_recover_but_never_kill() {
    sim::test_timeout(1800);
    let mut sc = scenario("gate-g", 13).quota(3_000, 20);
    sc.until = Some(SIM_START + DAY + 2 * HOUR);
    sc.world = WorldConfig {
        owner_per_hour: 6.0,
        peer_per_hour: 10.0,
        ..busy_world(13, DAY + 2 * HOUR)
    };
    let a = Some("mock/a".to_string());
    sc.faults = vec![
        // The primary model's provider rate-limits, with Retry-After.
        Fault {
            from: at(0, 20),
            to: at(0, 35),
            kind: FaultKind::ProviderRateLimit {
                retry_after_ms: 30_000,
            },
            model: a.clone(),
        },
        // A 5xx burst on the primary.
        Fault {
            from: at(1, 30),
            to: at(1, 34),
            kind: FaultKind::ServerError,
            model: a.clone(),
        },
        // The platform's own 429, with a reset two minutes on.
        Fault {
            from: at(2, 30),
            to: at(2, 40),
            kind: FaultKind::PlatformRateLimit {
                reset_in_ms: 2 * MINUTE,
            },
            model: None,
        },
        // Nothing answers for ten minutes.
        Fault {
            from: at(3, 0),
            to: at(3, 10),
            kind: FaultKind::Outage,
            model: None,
        },
        // The credential is refused for forty minutes.
        Fault {
            from: at(4, 0),
            to: at(4, 40),
            kind: FaultKind::Auth,
            model: None,
        },
        // The account's daily quota runs out: wait for UTC midnight.
        Fault {
            from: at(6, 0),
            to: SIM_START + DAY,
            kind: FaultKind::DailyQuota,
            model: None,
        },
    ];
    // Hours of turns: memory is exercised elsewhere.
    sc.memory = false;
    let out = sim::run(sc);
    // The run ended only by its limit.
    let exited = !matches!(&out.why, rung_host::stop::Why::Stopped { by } if by == "limit");
    assert_gate(&gates::g_g(&out.lines, exited));
    assert_gate(&gates::g_k(&out.lines));
}
