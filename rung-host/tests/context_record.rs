//! S4: the request record (layers, whole, redacted) and the context door.
//! A sentinel secret is never put in an assertion message.

mod common;

use std::sync::{Arc, Mutex};

use rung_host::clock::{Clock, HOUR, SimClock};
use rung_host::engine::{EngineTurn, TurnEngine, TurnRequest};
use rung_host::sim::{self, MockEngine, SIM_START};

/// The mock, remembering the canonical bytes of every request it was handed
/// (the first call of each turn: the thread as the host assembled it).
struct Spy {
    inner: Arc<MockEngine>,
    seen: Mutex<Vec<(u64, Vec<u8>)>>,
}

impl TurnEngine for Spy {
    fn name(&self) -> &str {
        "mock"
    }
    fn turn(&self, req: TurnRequest) -> EngineTurn {
        let bytes = rung_host::pack::request_bytes(
            &req.tools.definitions(),
            &req.thread.system_prompt,
            &req.thread.messages,
        );
        self.seen.lock().unwrap().push((req.turn, bytes));
        self.inner.turn(req)
    }
}

fn spied(sc: &mut sim::Scenario) -> Arc<Spy> {
    let clock: Arc<dyn Clock> = Arc::new(SimClock::new(SIM_START));
    sc.clock = Some(clock.clone());
    let mock = Arc::new(MockEngine::new(
        sc.mock.clone(),
        clock,
        sim::FaultInjector::new(Vec::new()),
    ));
    let spy = Arc::new(Spy {
        inner: mock,
        seen: Mutex::new(Vec::new()),
    });
    sc.engine = Some(spy.clone());
    spy
}

/// One digest per turn of a seeded busy run.
fn digests(tag: &str, budget: Option<usize>, turns: u64) -> (Vec<String>, usize) {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard(tag);
    let mut sc = sim::Scenario::new(guard.path(), 11);
    sc.world = common::busy_world(11, 3 * HOUR);
    sc.max_turns = Some(turns);
    if let Some(b) = budget {
        sc.config.epoch_budget_tokens = b;
    }
    let spy = spied(&mut sc);
    let out = sim::run(sc);
    let rolls = out
        .lines
        .iter()
        .filter(|l| l.kind == "epoch.rollover")
        .count();
    let seen = spy.seen.lock().unwrap();
    // the scratch directory's name (it carries the process id) is not part
    // of what is being pinned
    let dir = guard.path().to_string_lossy().to_string();
    let d = seen
        .iter()
        .map(|(t, b)| {
            let text = String::from_utf8_lossy(b).replace(&dir, "<DIR>");
            format!("{t}:{}", rung_host::canon::hash(text.as_bytes()))
        })
        .collect();
    (d, rolls)
}

/// The request bytes the provider is handed do not change: one digest per
/// turn of a seeded busy run, folded to one, pinned from the host before the
/// request record existed (a run with no rollover after the first, and one
/// with several).
#[test]
fn the_request_bytes_are_unchanged() {
    let (plain, _) = digests("ctx-golden", None, 14);
    let (rolled, rolls) = digests("ctx-golden-roll", Some(6_000), 14);
    assert!(rolls >= 3, "the second run rolls over");
    let fold = |d: &[String]| rung_host::canon::hash(d.join(",").as_bytes());
    assert_eq!(fold(&plain), "87e3a16912b958ae686c2699db0c2e96");
    assert_eq!(fold(&rolled), "1a0a338bfe6d60ed480273722384e4cb");
}
