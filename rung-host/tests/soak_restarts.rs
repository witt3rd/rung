//! Long-run soak: 24 simulated hours of random stimuli, provider faults and
//! abrupt restarts (the host is dropped mid-turn or mid-accept with no
//! cleanup, as `kill -9` leaves it), on the simulated clock and the mock
//! engine. The record must stay consistent: no stimulus lost or duplicated,
//! no turn number reused, every turn ended at most once, time never backwards.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use common::*;
use rung_host::clock::{Clock, DAY, MINUTE, SECOND, SimClock};
use rung_host::engine::{EngineTurn, TurnEngine, TurnRequest};
use rung_host::gates;
use rung_host::inbox::{Item, Source};
use rung_host::sim::{
    self, FakeWorld, Fault, FaultInjector, FaultKind, MockEngine, Rng, SIM_START, WorldConfig,
};
use rung_host::state::State;

/// Counts down shared "kill points"; at zero the next point panics.
struct Reaper(AtomicI64);

impl Reaper {
    fn point(&self, what: &str) {
        if self.0.fetch_sub(1, Ordering::SeqCst) == 1 {
            panic!("soak kill: {what}");
        }
    }
}

struct Doomed {
    inner: Arc<MockEngine>,
    reaper: Arc<Reaper>,
}

impl TurnEngine for Doomed {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn turn(&self, req: TurnRequest) -> EngineTurn {
        // After `turn.started` is recorded, before `turn.ended`.
        self.reaper.point("mid-turn");
        self.inner.turn(req)
    }
}

struct DoomedWorld {
    inner: FakeWorld,
    reaper: Arc<Reaper>,
}

impl Source for DoomedWorld {
    fn poll(&mut self, now: i64, seen: &BTreeSet<String>) -> Vec<Item> {
        self.inner.poll(now, seen)
    }
    fn accepted(&mut self, item: &Item) {
        // The item is recorded and fsynced; the source has not heard.
        self.inner.accepted(item);
        self.reaper.point("after accept");
    }
    fn next_at(&self) -> Option<i64> {
        self.inner.next_at()
    }
}

fn faults(rng: &mut Rng) -> Vec<Fault> {
    let mut out = Vec::new();
    let mut t = SIM_START + 10 * MINUTE;
    while t < SIM_START + DAY {
        let len = (1 + rng.below(20)) as i64 * MINUTE;
        let kind = match rng.below(6) {
            0 => FaultKind::ProviderRateLimit {
                retry_after_ms: 5_000 + rng.below(60_000),
            },
            1 => FaultKind::ServerError,
            2 => FaultKind::PlatformRateLimit {
                reset_in_ms: 2 * MINUTE,
            },
            3 => FaultKind::Outage,
            4 => FaultKind::Auth,
            _ => FaultKind::CacheEvict,
        };
        out.push(Fault {
            from: t,
            to: t + len,
            kind,
            model: None,
        });
        t += len + (20 + rng.below(100)) as i64 * MINUTE;
    }
    out
}

#[test]
fn a_simulated_day_of_kills_faults_and_stimuli_loses_and_repeats_nothing() {
    sim::test_timeout(1800);
    std::panic::set_hook(Box::new(|i| {
        let m = i.to_string();
        if !m.contains("soak kill") {
            eprintln!("{m}");
        }
    }));
    let seed = 24;
    let guard = sim::temp_dir_guard("soak-restarts");
    let dir = guard.path().to_path_buf();
    let clock = Arc::new(SimClock::new(SIM_START));
    let mut rng = Rng::new(seed);
    let fault_plan = faults(&mut rng);
    let end = SIM_START + DAY;
    let world = WorldConfig {
        owner_per_hour: 4.0,
        peer_per_hour: 12.0,
        ..busy_world(seed, DAY)
    };
    let mut kills = 0usize;
    let mut segments = 0usize;
    loop {
        segments += 1;
        assert!(segments < 2_000, "soak made no progress");
        let reaper = Arc::new(Reaper(AtomicI64::new(3 + rng.below(25) as i64)));
        let mut sc = sim::Scenario::new(&dir, seed);
        sc.memory = false;
        // Minute-scale calls keep a simulated day to about a thousand turns.
        sc.mock.call_ms = (20 * SECOND, 90 * SECOND);
        sc.until = Some(end);
        sc.clock = Some(clock.clone());
        sc.world = WorldConfig::quiet(seed, SIM_START, 1);
        sc.sources.push(Box::new(DoomedWorld {
            inner: FakeWorld::new(&world),
            reaper: reaper.clone(),
        }));
        let mock = Arc::new(MockEngine::new(
            sc.mock.clone(),
            clock.clone(),
            FaultInjector::new(fault_plan.clone()),
        ));
        sc.engine = Some(Arc::new(Doomed {
            inner: mock,
            reaper,
        }));
        let (host, rec, _m) = sim::build(sc);
        match catch_unwind(AssertUnwindSafe(|| host.run(rec))) {
            Ok(_) => break,
            Err(_) => {
                kills += 1;
                drop(host);
                // Downtime before the supervisor restarts it.
                clock.advance(SECOND + rng.below(5 * MINUTE as u64) as i64);
            }
        }
    }
    let lines = rung_host::record::Record::read_dir(dir.join("record")).expect("read record");
    eprintln!("soak: {kills} kills, {} lines", lines.len());

    // Sim time covered the whole day.
    assert!(clock.now() >= end, "run ended early at {}", clock.now());
    assert!(kills >= gates::G_I_KILLS, "only {kills} kills");

    // Provider faults actually bit.
    assert!(
        lines.iter().any(|l| l.kind == "degraded"),
        "no fault degraded the host"
    );

    // The record is gapless and time never runs backwards.
    for (i, w) in lines.windows(2).enumerate() {
        assert_eq!(w[1].seq, w[0].seq + 1, "seq gap at line {i}");
        assert!(
            w[1].at >= w[0].at,
            "time went backwards at seq {}",
            w[1].seq
        );
    }

    // No turn number reused; each turn ended at most once, only after it started.
    let mut started: BTreeMap<u64, usize> = BTreeMap::new();
    let mut ended: BTreeMap<u64, usize> = BTreeMap::new();
    let mut open: Option<u64> = None;
    for l in &lines {
        match l.kind.as_str() {
            "turn.started" => {
                let n = l.u64("turn");
                *started.entry(n).or_default() += 1;
                open = Some(n);
            }
            "turn.ended" => {
                let n = l.u64("turn");
                assert_eq!(open, Some(n), "turn {n} ended without being the open turn");
                *ended.entry(n).or_default() += 1;
                open = None;
            }
            "host.start" => open = None,
            _ => {}
        }
    }
    let dup_start: Vec<_> = started.iter().filter(|(_, c)| **c > 1).collect();
    let dup_end: Vec<_> = ended.iter().filter(|(_, c)| **c > 1).collect();
    assert!(dup_start.is_empty(), "turn numbers reused: {dup_start:?}");
    assert!(dup_end.is_empty(), "turns ended twice: {dup_end:?}");
    let lost = started.len() - ended.len();
    assert!(
        lost <= kills,
        "{lost} turns started but never ended; {kills} kills"
    );
    eprintln!("soak: {} turns, {lost} cut off by a kill", started.len());

    // No stimulus lost or duplicated, projections rebuild, nothing spent.
    let replayed = State::replay_hashes(&lines);
    let gi = gates::g_i(&lines, kills, &replayed);
    assert_gate_in(&dir, &lines, &gi);
    assert_gate_in(&dir, &lines, &gates::g_k(&lines));
}
