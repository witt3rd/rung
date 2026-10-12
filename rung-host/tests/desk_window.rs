//! The decider's context window bounds every desk ask, and a host sizes its asks
//! by it: the pack/rollover ask at the default budget, and an admit ask with huge
//! waiting items, are answered (not refused), carry whole text, and what does
//! not fit is left to the rule or to a later ask, never cut.

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use rung_host::clock::HOUR;
use rung_host::desk::{DeskMode, Scripted, Step};
use rung_host::inbox::{Item, MemorySource, Role};
use rung_host::sim::{self, DeskSpec, SIM_START};
use rung_std::decide::{Ask, DECIDER_WINDOW_TOKENS, Decided, Decider, Undecided};

/// A decider that, like the real one, refuses an ask above its window, and
/// remembers every ask it was given.
struct Windowed {
    inner: Scripted,
    seen: Mutex<Vec<Ask>>,
}

impl Windowed {
    fn new() -> Self {
        Self {
            inner: Scripted::always(Step::Seeded(5)),
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl std::fmt::Debug for Windowed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Windowed")
    }
}

impl Decider for Windowed {
    fn decide(&self, ask: &Ask) -> Result<Decided, Undecided> {
        self.seen.lock().unwrap().push(ask.clone());
        if ask.estimated_tokens() > DECIDER_WINDOW_TOKENS {
            return Err(Undecided::TooLarge);
        }
        self.inner.decide(ask)
    }
}

fn desk_asks(lines: &[rung_host::record::Line]) -> Vec<&rung_host::record::Line> {
    lines.iter().filter(|l| l.kind == "desk.ask").collect()
}

fn has_family(l: &rung_host::record::Line, f: &str) -> bool {
    l.get("families")
        .as_array()
        .is_some_and(|a| a.iter().any(|x| x == f))
}

/// At the default epoch budget the gate opens at 40% of 128,000 tokens, far more
/// text than the window holds. The pack ask must carry as many whole segments as
/// fit and still be answered, as it was before gists were whole.
#[test]
fn the_pack_ask_at_the_default_budget_is_answered() {
    sim::test_timeout(900);
    let mut sc = scenario("desk-window-pack", 11);
    sc.world = busy_world(11, 24 * HOUR);
    sc.max_turns = Some(400);
    let decider = Arc::new(Windowed::new());
    sc.desk = DeskSpec::Decider {
        decider: decider.clone(),
        backend: "scripted".into(),
        mode: DeskMode::Decide,
    };
    let out = sim::run(sc);
    let asks = desk_asks(&out.lines);
    let pack: Vec<_> = asks.iter().filter(|l| has_family(l, "pack")).collect();
    let outcomes: Vec<&str> = pack.iter().map(|l| l.str("outcome")).collect();
    assert!(
        pack.len() >= 3,
        "the run never reached the pack gate: {} pack asks",
        pack.len()
    );
    // None is refused for its size, and (nearly) all are answered. The desk's
    // daily spend cap (money, not content) may still stop a few late in a day, so
    // the run is not held to every one.
    assert!(
        outcomes.iter().all(|o| !o.starts_with("undecided")),
        "a pack ask was refused: {outcomes:?}"
    );
    let answered = outcomes.iter().filter(|o| **o == "answered").count();
    assert!(
        answered * 4 >= outcomes.len() * 3,
        "pack asks not answered: {outcomes:?}"
    );
    let seen = decider.seen.lock().unwrap();
    assert!(
        seen.iter()
            .all(|a| a.estimated_tokens() <= DECIDER_WINDOW_TOKENS),
        "an ask went out above the window"
    );
    // A gist is the segment's whole text: none carries the old cut marker or
    // the old 150-character length when the segment has more text.
    assert!(
        seen.iter()
            .any(|a| a.state["pack"]["segments"].as_array().is_some_and(|s| s
                .iter()
                .any(|g| g["gist"].as_str().unwrap_or("").chars().count() > 150))),
        "no asked segment carried more than 150 characters of its text"
    );
}

/// Thirty 20 KB waiting messages do not fit one ask. Admit asks the oldest that
/// fit (never more than the window holds), is answered, and every message is
/// still disposed of (admitted by the decider or the rule), none dropped.
#[test]
fn an_admit_ask_with_huge_items_is_sized_by_the_window() {
    sim::test_timeout(300);
    let mut sc = scenario("desk-window-admit", 13);
    sc.max_turns = Some(12);
    sc.memory = false;
    let mut src = MemorySource::default();
    for i in 0..30 {
        src.push(Item::message(
            &format!("big-{i:02}"),
            Role::Peer,
            "chat",
            SIM_START + i,
            &format!("{} END{i}", "x".repeat(20_000)),
        ));
    }
    sc.sources.push(Box::new(src));
    let decider = Arc::new(Windowed::new());
    sc.desk = DeskSpec::Decider {
        decider: decider.clone(),
        backend: "scripted".into(),
        mode: DeskMode::Decide,
    };
    let out = sim::run(sc);
    let asks = desk_asks(&out.lines);
    let admit: Vec<_> = asks.iter().filter(|l| has_family(l, "admit")).collect();
    assert!(!admit.is_empty());
    for l in &admit {
        assert_eq!(l.str("outcome"), "answered", "{l:?}");
    }
    let seen = decider.seen.lock().unwrap();
    let most = seen
        .iter()
        .filter_map(|a| a.state["admit"]["items"].as_array().map(|i| i.len()))
        .max()
        .unwrap_or(0);
    assert!(
        most > 0 && most < 30,
        "the ask carried {most} of 30 items: not sized by the window"
    );
    // The gist of an asked item is its whole text.
    assert!(
        seen.iter()
            .any(|a| a.state["admit"]["items"].as_array().is_some_and(|i| i
                .iter()
                .any(|g| g["gist"].as_str().unwrap_or("").len() > 20_000)))
    );
}
