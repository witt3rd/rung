//! No caps on the text the host puts in the agent's prompt or in a decider's
//! state: the register lists, an admitted message, the epoch outline, a
//! memory candidate and a gist are whole. The one bound is the decider's
//! context window, and an ask that would pass it asks fewer items (the rest
//! wait), never cuts one.

use rung_host::clock::Millis;
use rung_host::desk::DeskState;
use rung_host::inbox::{Item, Role, gist};
use rung_host::kernel::KernelState;
use rung_host::record::Line;
use rung_host::registers::{Registers, Todo};
use rung_host::render::{HeaderCtx, header, material};
use rung_host::state::PackMeta;

const NOW: Millis = 1_790_000_000_000;

fn line(seq: u64, kind: &str, body: serde_json::Value) -> Line {
    let mut v = body;
    v["seq"] = seq.into();
    v["at"] = NOW.into();
    v["kind"] = kind.into();
    Line::parse(&v.to_string()).expect("a line")
}

#[test]
fn every_open_todo_and_its_whole_text_reach_the_agent() {
    let mut reg = Registers::default();
    for i in 0..200 {
        reg.todo.insert(
            format!("t{i:03}"),
            Todo {
                text: format!("todo {i} {} END{i}", "w".repeat(300)),
                created_at: NOW + i,
                done: false,
                priority: None,
            },
        );
    }
    let m = material(&reg, &KernelState::default(), NOW + 1000);
    for i in 0..200 {
        assert!(m.contains(&format!("END{i}")), "todo {i} is missing or cut");
    }
    assert!(!m.contains('…'));
}

#[test]
fn a_20kb_admitted_message_is_whole_in_the_header() {
    let body = format!("{} END-OF-MESSAGE", "m".repeat(20_000));
    let item = Item::message("m1", Role::Peer, "chat", NOW, &body);
    let h = HeaderCtx {
        turn: 1,
        kind: rung_host::kernel::TurnKind::Responding,
        now: NOW,
        since_external: None,
        quota: None,
        model: "m",
        rung: 0,
        enabled: &["core".into()],
        admitted: vec![&item],
        digests: vec![&item],
        commitment: None,
        turns_since_progress: 0,
        resumed: false,
        material: None,
        recall: None,
        expectations: None,
        calendar: None,
        note_line: false,
        notices: vec![],
    };
    let s = header(&h);
    assert_eq!(s.matches(&body).count(), 2, "the message is cut");
}

#[test]
fn the_outline_keeps_every_turn_whole() {
    let mut p = PackMeta::default();
    for t in 1..=60u64 {
        p.apply(&line(
            t,
            "turn.ended",
            serde_json::json!({"turn": t, "turn_kind": "free", "status": "completed",
                "final_text": format!("{} END{t}", "o".repeat(500))}),
        ));
    }
    assert_eq!(p.outline.len(), 60, "turns were dropped from the outline");
    for t in 1..=60 {
        assert!(
            p.outline[t - 1].ends_with(&format!("END{t}")),
            "turn {t} cut"
        );
    }
}

#[test]
fn a_candidate_is_kept_whole_and_none_is_dropped() {
    let mut d = DeskState::default();
    for t in 1..=80u64 {
        d.apply(&line(
            t,
            "kernel.trace",
            serde_json::json!({"turn": t, "what_pulled": format!("{} END{t}", "c".repeat(3000)),
                "where_it_went": "x", "still_thinking": "y"}),
        ));
    }
    assert_eq!(d.candidates.len(), 80, "candidates were dropped");
    assert!(d.candidates[0].text.contains("END1 "), "the text is cut");
    assert!(d.candidates[0].gist.contains("END1 "), "the gist is cut");
}

#[test]
fn a_gist_is_not_cut() {
    let g = gist(&"g ".repeat(5000));
    assert_eq!(g.matches('g').count(), 5000);
}

#[test]
fn an_ask_carries_the_most_items_the_window_holds() {
    use rung_host::desk::fit_count;
    // 10 units of room: each entry takes 3.
    assert_eq!(fit_count(8, |n| n * 3 <= 10), 3);
    assert_eq!(fit_count(2, |n| n * 3 <= 10), 2);
    assert_eq!(fit_count(0, |_| false), 0);
    // Nothing fits: one is still asked, and the decider refuses it whole.
    assert_eq!(fit_count(5, |_| false), 1);
}
