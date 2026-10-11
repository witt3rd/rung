//! No caps on content the host stores: the record's `turn.ended` carries the
//! whole answer, whatever its size.

mod common;

use std::sync::Arc;

use rung_host::clock::{Clock, SimClock};
use rung_host::engine::{EngineTurn, TurnEngine, TurnRequest};
use rung_host::sim::{self, MockEngine, SIM_START};

/// The mock, with every final answer replaced by a huge one.
struct Verbose {
    inner: Arc<MockEngine>,
    text: String,
}

impl TurnEngine for Verbose {
    fn name(&self) -> &str {
        "mock"
    }
    fn turn(&self, req: TurnRequest) -> EngineTurn {
        let mut t = self.inner.turn(req);
        if t.final_text.is_some() {
            t.final_text = Some(self.text.clone());
        }
        t
    }
}

#[test]
fn a_long_answer_is_recorded_whole() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("whole-answer");
    let clock: Arc<dyn Clock> = Arc::new(SimClock::new(SIM_START));
    let mut sc = sim::Scenario::new(guard.path(), 3);
    sc.memory = false;
    sc.max_turns = Some(1);
    sc.clock = Some(clock.clone());
    let mock = Arc::new(MockEngine::new(
        sc.mock.clone(),
        clock,
        sim::FaultInjector::new(Vec::new()),
    ));
    // 50 KB of multi-byte text, ending in a sentinel.
    let text = format!("{}END-OF-ANSWER", "é".repeat(25_000));
    sc.engine = Some(Arc::new(Verbose {
        inner: mock,
        text: text.clone(),
    }));
    let out = sim::run(sc);
    let ended: Vec<_> = out
        .lines
        .iter()
        .filter(|l| l.kind == "turn.ended")
        .collect();
    assert!(!ended.is_empty(), "no turn ended");
    assert_eq!(ended[0].str("final_text"), text);
    // And the same off the disk, as a reader of the record sees it.
    let lines = rung_host::record::Record::read_dir(guard.path().join("record")).unwrap();
    let on_disk = lines.iter().find(|l| l.kind == "turn.ended").unwrap();
    assert_eq!(on_disk.str("final_text"), text);
}
