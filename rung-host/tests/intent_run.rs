//! S2: the intents in a whole simulated run (scripted engine, simulated clock: nothing
//! sleeps). A `steer` is read at the first boundary after it arrives and is marked in the
//! header the model reads; an item sent as `free` is not read while the agent is committed
//! and is read once it is free; the intent is in the record and survives a restart (the
//! inbox is a fold of the record). Written first, against a host with no intent.

mod common;

use common::*;
use rung_host::clock::SECOND;
use rung_host::inbox::{Intent, Item, MemorySource, Role};
use rung_host::record::Line;
use rung_host::sim::{self, SIM_START};

/// A run in which the agent commits at its first free turn (and again after each release):
/// the scripted engine commits at once, so most turns are committed ones.
fn committing(name: &str, items: Vec<Item>, turns: u64) -> sim::RunOutput {
    sim::test_timeout(120);
    let mut sc = scenario(name, 3);
    sc.max_turns = Some(turns);
    sc.mock.p_commit = 1.0;
    sc.mock.p_long_work = 0.0;
    sc.mock.p_expect = 0.0;
    let mut src = MemorySource::default();
    for i in items {
        src.push(i);
    }
    sc.sources.push(Box::new(src));
    sim::run(sc)
}

fn owner(id: &str, intent: Intent, at: i64, text: &str) -> Item {
    let mut i = Item::message(id, Role::Owner, "owner", at, text);
    i.intent = intent;
    i
}

fn kind_of(lines: &[Line], turn: u64) -> String {
    lines
        .iter()
        .find(|l| l.kind == "turn.started" && l.u64("turn") == turn)
        .map(|l| l.str("turn_kind").to_string())
        .unwrap_or_default()
}

/// The turn that admitted `id`, if any.
fn admitted_in(lines: &[Line], id: &str) -> Option<u64> {
    lines
        .iter()
        .find(|l| {
            l.kind == "stimulus.admitted"
                && l.get("ids")
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x == id))
        })
        .map(|l| l.u64("turn"))
}

fn seq_of<'a>(lines: &'a [Line], kind: &str, pred: impl Fn(&Line) -> bool) -> Option<&'a Line> {
    lines.iter().find(|l| l.kind == kind && pred(l))
}

#[test]
fn a_steer_is_read_at_the_first_boundary_after_it_arrives_and_is_marked() {
    let out = committing(
        "s2-run-steer",
        vec![owner(
            "st-1",
            Intent::Steer,
            SIM_START + 2 * SECOND,
            "use the other table",
        )],
        14,
    );
    let t = admitted_in(&out.lines, "st-1").expect("the steer was read");
    let accepted = seq_of(&out.lines, "stimulus.accepted", |l| {
        l.get("item")["id"] == "st-1"
    })
    .unwrap();
    // the first boundary after it was recorded: the turn that follows its acceptance
    let first_after = out
        .lines
        .iter()
        .find(|l| l.kind == "turn.started" && l.seq > accepted.seq)
        .map(|l| l.u64("turn"))
        .unwrap();
    assert_eq!(t, first_after, "read at the next boundary");
    // committed work was displaced: the steer is read whatever the agent was doing
    assert_eq!(kind_of(&out.lines, t), "responding");
    assert!(
        (1..t).any(|n| kind_of(&out.lines, n) == "committed"),
        "the agent was committed before the steer came"
    );
    // the record line carries the intent
    assert_eq!(accepted.get("item")["intent"], "steer");
    // the header the model reads shows it as a steer, not as a task
    let log = seq_of(&out.lines, "turn.log", |l| l.u64("turn") == t).unwrap();
    let header = log.get("header").as_str().unwrap_or("");
    let row = header
        .lines()
        .find(|l| l.contains("st-1"))
        .expect("the item is in the header");
    assert!(row.contains("steer"), "{row}");
    assert!(row.contains("use the other table"), "{row}");
}

#[test]
fn a_next_item_is_not_marked_as_a_steer() {
    let out = committing(
        "s2-run-next-unmarked",
        vec![owner(
            "nx-1",
            Intent::Next,
            SIM_START + 2 * SECOND,
            "a task",
        )],
        14,
    );
    let t = admitted_in(&out.lines, "nx-1").unwrap();
    let log = seq_of(&out.lines, "turn.log", |l| l.u64("turn") == t).unwrap();
    let row = log
        .get("header")
        .as_str()
        .unwrap()
        .lines()
        .find(|l| l.contains("nx-1"))
        .unwrap()
        .to_string();
    assert!(!row.contains("steer"), "{row}");
}

#[test]
fn a_free_item_waits_while_the_agent_is_committed_and_is_read_when_it_is_free() {
    let out = committing(
        "s2-run-free",
        vec![owner(
            "fr-1",
            Intent::Free,
            SIM_START + 2 * SECOND,
            "tidy the notes file",
        )],
        40,
    );
    let accepted = seq_of(&out.lines, "stimulus.accepted", |l| {
        l.get("item")["id"] == "fr-1"
    })
    .unwrap();
    assert_eq!(accepted.get("item")["intent"], "free");
    let t = admitted_in(&out.lines, "fr-1").expect("the item was read once the agent was free");
    // every turn between its arrival and its reading was committed work, and left alone
    let between: Vec<u64> = out
        .lines
        .iter()
        .filter(|l| l.kind == "turn.started" && l.seq > accepted.seq && l.u64("turn") < t)
        .map(|l| l.u64("turn"))
        .collect();
    assert!(!between.is_empty(), "the agent was busy when it arrived");
    for n in &between {
        assert_eq!(kind_of(&out.lines, *n), "committed", "turn {n}");
        let decided = seq_of(&out.lines, "decision.admit", |l| l.u64("turn") == *n);
        if let Some(d) = decided {
            assert_ne!(d.get("choice")["forms"]["fr-1"], "now", "turn {n}");
        }
    }
    // read in free time: the commitment had been released before the boundary that read it
    let admitting = out
        .lines
        .iter()
        .find(|l| l.kind == "stimulus.admitted" && l.u64("turn") == t)
        .unwrap();
    assert!(
        out.lines
            .iter()
            .any(|l| l.kind == "kernel.release" && l.seq < admitting.seq && l.seq > accepted.seq),
        "the agent let go of its project before the item was read"
    );
}

#[test]
fn the_intent_survives_a_restart() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("s2-run-restart");
    let build = |max_turns: u64, items: Vec<Item>| {
        let mut sc = sim::Scenario::new(guard.path(), 3);
        sc.max_turns = Some(max_turns);
        sc.mock.p_commit = 1.0;
        sc.mock.p_long_work = 0.0;
        let mut src = MemorySource::default();
        for i in items {
            src.push(i);
        }
        sc.sources.push(Box::new(src));
        sim::build(sc)
    };
    // four turns: the agent commits, then the owner's two items arrive during committed work.
    // A steer would be read at the next boundary, so it is the free one that is still waiting
    // when the process ends; both were accepted with their intent.
    {
        let (host, rec, _) = build(
            4,
            vec![
                owner("a-free", Intent::Free, SIM_START + SECOND, "later"),
                owner("a-steer", Intent::Steer, SIM_START + SECOND, "now"),
            ],
        );
        let _ = host.run(rec);
    }
    // a new process over the same state: the inbox is rebuilt from the record
    let (host, _, _) = build(0, vec![]);
    let st = host.core.state();
    let free = st
        .inbox
        .pending
        .get("a-free")
        .expect("the free item still waits");
    assert_eq!(
        free.item.intent,
        Intent::Free,
        "the intent is the one that was sent"
    );
    assert!(
        !st.inbox.pending.contains_key("a-steer"),
        "the steer was read (it is not waiting)"
    );
    assert!(st.inbox.seen.contains("a-steer"));
}

#[test]
fn a_long_steer_reaches_the_model_whole() {
    let text = format!("{} END-OF-STEER", "plenty of words ".repeat(900));
    let out = committing(
        "s2-run-steer-whole",
        vec![owner(
            "st-long",
            Intent::Steer,
            SIM_START + 2 * SECOND,
            &text,
        )],
        14,
    );
    let t = admitted_in(&out.lines, "st-long").unwrap();
    let log = seq_of(&out.lines, "turn.log", |l| l.u64("turn") == t).unwrap();
    let header = log.get("header").as_str().unwrap();
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        header.contains(&one_line),
        "the header holds the message whole"
    );
    assert!(!header.contains('…'));
}

/// The header lists the owner's items first (a `next` or a `steer`, then a `free`), then a
/// peer's, whatever their ids sort like. All of them are read at one boundary: the agent is
/// in free time and at a break when they arrive, so the desk shows each.
#[test]
fn the_header_lists_owner_items_before_a_peers() {
    sim::test_timeout(120);
    let mut sc = scenario("s2-run-header-order", 3);
    sc.max_turns = Some(2);
    sc.mock.p_commit = 0.0;
    let mut src = MemorySource::default();
    for i in [
        Item::message(
            "a-peer",
            Role::Peer,
            "peer:atlas",
            SIM_START,
            "a note from atlas",
        ),
        owner("b-free", Intent::Free, SIM_START, "free words"),
        owner("c-steer", Intent::Steer, SIM_START, "steer words"),
        owner("d-next", Intent::Next, SIM_START, "task words"),
    ] {
        src.push(i);
    }
    sc.sources.push(Box::new(src));
    let out = sim::run(sc);
    let t = admitted_in(&out.lines, "a-peer").expect("the peer's note was read");
    for id in ["b-free", "c-steer", "d-next"] {
        assert_eq!(
            admitted_in(&out.lines, id),
            Some(t),
            "{id} was read at the same boundary"
        );
    }
    let log = seq_of(&out.lines, "turn.log", |l| l.u64("turn") == t).unwrap();
    let ids: Vec<&str> = log
        .get("header")
        .as_str()
        .unwrap()
        .lines()
        .filter(|l| l.starts_with("  ["))
        .filter_map(|l| {
            l.trim_start()
                .strip_prefix("[")
                .and_then(|r| r.split_once('/'))
        })
        .filter_map(|(_, r)| r.split_whitespace().next())
        .collect();
    assert_eq!(ids, ["c-steer", "d-next", "b-free", "a-peer"]);
}

/// A wait the owner may cut (the host is pacing its requests) is cut by a task, never by an
/// item the owner asked to wait for free time: that one would end the wait at once, over
/// and over, since the desk holds it while the agent is committed.
#[test]
fn a_free_item_does_not_end_a_paced_wait() {
    sim::test_timeout(30);
    let mut sc = scenario("s2-run-paced", 3).quota(100_000, 20);
    sc.max_turns = Some(12);
    sc.mock.p_commit = 1.0;
    sc.mock.p_long_work = 0.0;
    let mut src = MemorySource::default();
    src.push(owner("pw-free", Intent::Free, SIM_START + 1_000, "later"));
    sc.sources.push(Box::new(src));
    let out = sim::run(sc);
    // each wait ran as long as it was declared to (`until`), none was cut short
    let mut open: Option<&Line> = None;
    let (mut waits, mut cut) = (0, 0);
    for l in &out.lines {
        match l.kind.as_str() {
            "degraded" if l.str("class") == "paced" => open = Some(l),
            "degraded.ended" if l.str("class") == "paced" => {
                let d = open.take().expect("a wait ended that was declared");
                waits += 1;
                if l.at < d.i64("until") {
                    cut += 1;
                }
            }
            _ => {}
        }
    }
    assert!(waits > 0, "the run was paced");
    assert_eq!(cut, 0, "{cut} of {waits} paced waits were cut short");
}

/// A steer that arrives while the agent is in a long tool call cuts that call (the tool
/// result says the owner is waiting) and is read at the very next boundary: the whole path,
/// with the host's own watch. A task and a steer behave alike here; an item sent as `free`
/// is the one that does not (the toolbox's own tests).
#[test]
fn a_steer_in_a_long_call_cuts_it_and_is_read_at_the_next_boundary() {
    sim::test_timeout(60);
    for (id, intent) in [("lc-steer", Intent::Steer), ("lc-free", Intent::Free)] {
        let mut sc = scenario(&format!("s2-run-long-{id}"), 3);
        sc.max_turns = Some(60);
        sc.mock.p_commit = 1.0;
        sc.mock.p_long_work = 1.0;
        sc.mock.p_expect = 0.0;
        // far enough in that the agent has been fetching for a while
        let mut src = MemorySource::default();
        src.push(owner(
            id,
            intent,
            SIM_START + 100 * SECOND,
            "look at the other table",
        ));
        sc.sources.push(Box::new(src));
        let out = sim::run(sc);
        let cuts: Vec<&Line> = out
            .lines
            .iter()
            .filter(|l| l.kind == "tool.refused" && l.str("why") == "owner_waiting")
            .collect();
        let overran = out
            .lines
            .iter()
            .filter(|l| l.kind == "tool.refused" && l.str("why") == "overran")
            .count();
        assert!(overran > 0, "{id}: the agent did long work");
        let accepted = seq_of(&out.lines, "stimulus.accepted", |l| {
            l.get("item")["id"] == id
        })
        .unwrap();
        if intent == Intent::Steer {
            assert_eq!(cuts.len(), 1, "{id}: one call was cut");
            let cut = cuts[0];
            assert!(cut.seq > accepted.seq, "it was cut by the steer");
            // read at the first boundary after the cut
            let next_turn = out
                .lines
                .iter()
                .find(|l| l.kind == "turn.started" && l.seq > cut.seq)
                .map(|l| l.u64("turn"))
                .unwrap();
            assert_eq!(admitted_in(&out.lines, id), Some(next_turn));
        } else {
            assert!(cuts.is_empty(), "{id}: a free item cuts nothing");
        }
    }
}

/// Only the owner's own words reach the model whole: another sender's text is bounded in the
/// header as it always was (this slice widens nothing for a sender that is not the owner).
#[test]
fn a_peers_long_text_is_still_bounded_in_the_header() {
    sim::test_timeout(120);
    let mut sc = scenario("s2-run-peer-bounded", 3);
    sc.max_turns = Some(2);
    sc.mock.p_commit = 0.0;
    let mut src = MemorySource::default();
    let text = format!("{} PEER-END", "peer words ".repeat(400));
    src.push(Item::message(
        "pr-long",
        Role::Peer,
        "peer:atlas",
        SIM_START,
        &text,
    ));
    sc.sources.push(Box::new(src));
    let out = sim::run(sc);
    let t = admitted_in(&out.lines, "pr-long").expect("read");
    let log = seq_of(&out.lines, "turn.log", |l| l.u64("turn") == t).unwrap();
    let header = log.get("header").as_str().unwrap();
    let row = header.lines().find(|l| l.contains("pr-long")).unwrap();
    assert!(row.ends_with('…'), "bounded with an ellipsis, as before");
    assert!(!row.contains("PEER-END"));
}
