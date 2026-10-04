#![allow(dead_code)]
//! Shared scenario pieces for the gate tests.

use rung_host::clock::{HOUR, MINUTE};
use rung_host::gates::GateResult;
use rung_host::sim::{Fault, FaultKind, SIM_START, Scenario, WorldConfig};

/// Print a gate's evidence, then assert it.
pub fn assert_gate(g: &GateResult) {
    eprintln!("GATE {}", g.summary());
    assert!(g.pass, "{} failed: {:?}\n{}", g.id, g.failures, g.summary());
}

/// A busy world: owner and peer Poisson traffic, bursts, world facts.
pub fn busy_world(seed: u64, horizon: i64) -> WorldConfig {
    WorldConfig {
        seed,
        start: SIM_START,
        horizon,
        owner_per_hour: 3.0,
        peer_per_hour: 8.0,
        peers: 4,
        bursts: vec![(SIM_START + 40 * MINUTE, 20), (SIM_START + 5 * HOUR, 20)],
        facts_per_hour: 4.0,
        fact_keys: vec!["build".into(), "weather".into()],
    }
}

/// Scripted provider trouble on the primary model and the cache.
pub fn cache_and_ladder_faults() -> Vec<Fault> {
    let mut f = Vec::new();
    for k in 1..12 {
        let at = SIM_START + k * 23 * MINUTE;
        f.push(Fault {
            from: at,
            to: at + 1,
            kind: FaultKind::CacheEvict,
            model: None,
        });
    }
    f.push(Fault {
        from: SIM_START + 2 * HOUR,
        to: SIM_START + 2 * HOUR + 5 * MINUTE,
        kind: FaultKind::ProviderRateLimit {
            retry_after_ms: 20_000,
        },
        model: Some("mock/a".into()),
    });
    f
}

pub fn scenario(name: &str, seed: u64) -> Scenario {
    let guard = rung_host::sim::temp_dir_guard(name);
    let mut sc = Scenario::new(guard.path(), seed);
    sc.cleanup = Some(guard);
    sc
}
