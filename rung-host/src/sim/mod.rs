//! The test world for slice 1: a scripted engine, a fake world and a fault
//! injector, all seeded and offline. Nothing here reaches a network or a
//! live model; it is the harness the gates run on, not a product path.

pub mod faults;
pub mod http;
pub mod mock;
pub mod world;

use std::path::PathBuf;
use std::sync::Arc;

use crate::calendar::Entry;
use crate::clock::{Clock, Millis, SimClock};
use crate::core::HostConfig;
use crate::desk::{DecisionDesk, DeskMode};
use crate::gates::Captured;
use crate::governor::Quota;
use crate::inbox::Source;
use crate::memory::MemoryHost;
use crate::presence::{Host, HostBuilder, Limits};
use crate::record::Line;
use crate::registers::{Expectation, Judge};
use crate::stop::{StopAuthority, Why};

pub use faults::{Fault, FaultInjector, FaultKind};
pub use mock::{MockConfig, MockEngine};
pub use world::{FakeWeb, FakeWorld, WorldConfig};

/// 2026-10-03T00:00:00Z: where simulated runs start.
pub const SIM_START: Millis = 1_790_985_600_000;

/// A small seeded generator (SplitMix64).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9e37_79b9_7f4a_7c15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// In [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// In [0, n).
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }
}

/// A judge disjoint from the agent: settles judged expectations by a hash
/// of their id (60% met).
#[derive(Debug)]
pub struct SeededJudge {
    pub name: String,
}

impl Judge for SeededJudge {
    fn name(&self) -> &str {
        &self.name
    }
    fn judge(&self, id: &str, _e: &Expectation) -> Option<bool> {
        let h = crate::canon::hash(id.as_bytes());
        Some(u64::from_str_radix(&h[..8], 16).unwrap_or(0) % 100 < 60)
    }
}

/// How the desk decides in a scenario.
pub enum DeskSpec {
    RuleOnly,
    Decider {
        decider: Arc<dyn rung_std::decide::Decider>,
        backend: String,
        mode: DeskMode,
    },
}

/// One seeded run.
pub struct Scenario {
    pub dir: PathBuf,
    pub seed: u64,
    pub start: Millis,
    pub max_turns: Option<u64>,
    pub until: Option<Millis>,
    pub world: WorldConfig,
    pub mock: MockConfig,
    pub faults: Vec<Fault>,
    pub desk: DeskSpec,
    pub config: HostConfig,
    pub memory: bool,
    pub seed_calendar: Vec<Entry>,
    /// Extra sources (a directory, a memory queue).
    pub sources: Vec<Box<dyn Source>>,
    /// Use this clock instead of a fresh `SimClock`.
    pub clock: Option<Arc<dyn Clock>>,
    pub stop: Option<Arc<StopAuthority>>,
    pub notifier: Option<crate::notify::Notifier>,
    /// Removes the directory when the test passes (see [`temp_dir_guard`]).
    pub cleanup: Option<TempDir>,
    /// Run this engine instead of the scripted mock (the real adapter
    /// against a loopback provider). The mock is still built, unused.
    pub engine: Option<Arc<dyn crate::engine::TurnEngine>>,
    /// Lists the router's models for the ladder's filter.
    pub lister: Option<Arc<dyn crate::ladder::Lister>>,
    /// The desk's ask timeout, instead of [`crate::desk::ASK_TIMEOUT`]
    /// (the one wall-clock input a simulated run has).
    pub desk_timeout: Option<std::time::Duration>,
}

impl Scenario {
    /// A quiet scenario in `dir`, seeded.
    pub fn new(dir: impl Into<PathBuf>, seed: u64) -> Self {
        let dir = dir.into();
        let mut config = HostConfig::new(dir.join("workspace"));
        config.ladder = vec!["mock/a".into(), "mock/b".into(), "mock/c".into()];
        config.seed = seed;
        config.seed_projects = vec![(
            "p1".into(),
            "anticipatory-model".into(),
            "the owner's seed: build your own predictive model of yourself and your world".into(),
        )];
        config.ceiling.insert("memory".into());
        Self {
            seed,
            start: SIM_START,
            max_turns: None,
            until: None,
            world: WorldConfig::quiet(seed, SIM_START, 0),
            mock: MockConfig {
                seed,
                ..MockConfig::default()
            },
            faults: Vec::new(),
            desk: DeskSpec::RuleOnly,
            config,
            memory: true,
            seed_calendar: Vec::new(),
            sources: Vec::new(),
            clock: None,
            stop: None,
            notifier: None,
            cleanup: None,
            engine: None,
            lister: None,
            desk_timeout: None,
            dir,
        }
    }

    pub fn quota(mut self, rpd: u64, rpm: u64) -> Self {
        self.config.governor.quota = Some(Quota {
            rpd,
            rpm,
            reserve: 0.25,
        });
        self
    }
}

/// What a run left.
pub struct RunOutput {
    pub lines: Vec<Line>,
    pub captured: Vec<Captured>,
    pub why: Why,
    pub host: Arc<Host>,
    pub mock: Arc<MockEngine>,
    /// Keeps the scenario directory until the output is dropped.
    pub cleanup: Option<TempDir>,
}

/// Build the host a scenario describes (without running it).
pub fn build(sc: Scenario) -> (Arc<Host>, crate::presence::Recovered, Arc<MockEngine>) {
    let clock: Arc<dyn Clock> = sc
        .clock
        .unwrap_or_else(|| Arc::new(SimClock::new(sc.start)));
    let mock = Arc::new(MockEngine::new(
        sc.mock,
        clock.clone(),
        FaultInjector::new(sc.faults),
    ));
    let mut world = sc.world;
    if world.horizon == 0 {
        world.horizon = 1;
    }
    let mut sources: Vec<Box<dyn Source>> = vec![Box::new(FakeWorld::new(&world))];
    sources.extend(sc.sources);
    let engine: Arc<dyn crate::engine::TurnEngine> = match sc.engine {
        Some(e) => e,
        None => mock.clone(),
    };
    let mut b = HostBuilder::new(sc.config, &sc.dir, clock, engine);
    if let Some(s) = sc.stop {
        b.stop = s;
    }
    if let Some(n) = sc.notifier {
        b.notifier = n;
    }
    b.desk = match sc.desk {
        DeskSpec::RuleOnly => DecisionDesk::rule_only(),
        DeskSpec::Decider {
            decider,
            backend,
            mode,
        } => DecisionDesk::new(Some(decider), &backend, mode),
    };
    if let Some(t) = sc.desk_timeout {
        b.desk.timeout = t;
    }
    if sc.memory {
        b.memory = Some(Arc::new(MemoryHost::baseline(
            &sc.dir.join("memory"),
            "host",
        )));
    }
    b.sources = sources;
    b.judge = Some(Arc::new(SeededJudge {
        name: "judge".into(),
    }));
    b.web = Arc::new(FakeWeb);
    b.limits = Limits {
        max_turns: sc.max_turns,
        until: sc.until,
    };
    b.seed_calendar = sc.seed_calendar;
    b.lister = sc.lister;
    let (host, rec) = Host::open(b).expect("open host");
    (host, rec, mock)
}

/// Run a scenario to its limit.
pub fn run(mut sc: Scenario) -> RunOutput {
    let cleanup = sc.cleanup.take();
    let (host, rec, mock) = build(sc);
    let why = host.run(rec);
    let lines = host.record_lines().expect("read record");
    let captured = std::mem::take(&mut *mock.captured.lock().expect("captured"));
    RunOutput {
        lines,
        captured,
        why,
        host,
        mock,
        cleanup,
    }
}

/// A fresh directory under the temp dir for a test.
pub fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rung-host-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp dir");
    d
}

/// A test directory removed on drop unless the test is panicking, so a
/// failed run keeps its evidence.
#[derive(Debug)]
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

/// Like [`temp_dir`], removed when the guard drops on a passing test.
pub fn temp_dir_guard(name: &str) -> TempDir {
    TempDir(temp_dir(name))
}

/// Fail the whole test process if it is still running after `secs`: a
/// deadlock or a wait on real time must not hang CI.
pub fn test_timeout(secs: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(secs));
        eprintln!("test timeout: still running after {secs} s");
        std::process::exit(101);
    });
}
