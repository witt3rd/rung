//! S2: what an owner's `intent` does at the decision desk. `next` and `steer` are
//! read at the next boundary, always; `free` waits for free time or a natural
//! break and never interrupts committed work, and the desk's limits and a firm
//! deadline still win over it. Written first, against a desk that forced every
//! owner item to `now`. No clock is read: the desk is a function of its context.

use rung_host::clock::{MINUTE, SECOND};
use rung_host::desk::HostQuestion;
use rung_host::desk::Knobs;
use rung_host::desk::admit::{Admit, AdmitCtx, AdmitInput, Form, Waiting};
use rung_host::inbox::{Intent, ItemKind, Role};
use rung_host::sim::SIM_START;
use serde_json::json;

const NOW: i64 = SIM_START + 90 * MINUTE;

fn owner(id: &str, intent: Intent, age_ms: i64) -> Waiting {
    Waiting {
        id: id.into(),
        kind: ItemKind::Peer,
        role: Role::Owner,
        at: NOW - age_ms,
        due: None,
        firm: false,
        intent,
    }
}

fn peer(id: &str, age_ms: i64) -> Waiting {
    Waiting {
        role: Role::Peer,
        ..owner(id, Intent::Next, age_ms)
    }
}

fn decide(
    waiting: Vec<Waiting>,
    committed: bool,
    at_break: bool,
    interrupts_last_hour: u32,
) -> rung_host::desk::admit::AdmitChoice {
    let k = Knobs::default();
    let input = AdmitInput {
        now: NOW,
        mode: json!({}),
        items: vec![],
        interrupts_last_hour,
        since_external_s: 5,
    };
    let ctx = AdmitCtx {
        now: NOW,
        waiting,
        at_break,
        committed,
    };
    Admit::guard(&input, Admit::rule(&input, &k, &ctx), &k, &ctx)
}

fn form(c: &rung_host::desk::admit::AdmitChoice, id: &str) -> Form {
    c.forms[id]
}

#[test]
fn next_and_steer_are_now_at_every_boundary_as_before() {
    for committed in [true, false] {
        let c = decide(
            vec![
                owner("n", Intent::Next, 5 * SECOND),
                owner("s", Intent::Steer, SECOND),
            ],
            committed,
            false,
            99,
        );
        assert_eq!(form(&c, "n"), Form::Now, "committed={committed}");
        assert_eq!(form(&c, "s"), Form::Now, "committed={committed}");
    }
}

#[test]
fn a_free_item_is_not_read_while_the_agent_is_committed() {
    let c = decide(vec![owner("f", Intent::Free, 5 * SECOND)], true, false, 0);
    assert_eq!(form(&c, "f"), Form::AtBreak);
    assert_eq!(c.now(), Vec::<String>::new());
    assert_eq!(c.interrupts, 0, "nothing interrupts the commitment");
}

#[test]
fn a_free_item_is_read_in_free_time_and_at_a_natural_break() {
    // free time, in the middle of a free session
    let c = decide(vec![owner("f", Intent::Free, 5 * SECOND)], false, false, 0);
    assert_eq!(form(&c, "f"), Form::Now);
    // free time, at a break
    let c = decide(vec![owner("f", Intent::Free, 5 * SECOND)], false, true, 0);
    assert_eq!(form(&c, "f"), Form::Now);
}

#[test]
fn a_free_item_does_not_wait_behind_next_items_when_it_is_read() {
    // read at one boundary with the others in free time; held when committed
    let w = || {
        vec![
            owner("f", Intent::Free, 9 * SECOND),
            owner("n", Intent::Next, 5 * SECOND),
        ]
    };
    let c = decide(w(), true, false, 0);
    assert_eq!((form(&c, "f"), form(&c, "n")), (Form::AtBreak, Form::Now));
    let c = decide(w(), false, true, 0);
    assert_eq!((form(&c, "f"), form(&c, "n")), (Form::Now, Form::Now));
}

#[test]
fn a_firm_deadline_on_a_free_item_still_wins() {
    let mut f = owner("f", Intent::Free, 10 * SECOND);
    f.firm = true;
    f.due = Some(NOW - SECOND);
    let c = decide(vec![f.clone()], true, false, 0);
    assert_eq!(form(&c, "f"), Form::Now, "a firm item that is due is read");
    // not yet due: it still waits
    f.due = Some(NOW + 5 * MINUTE);
    let c = decide(vec![f], true, false, 0);
    assert_eq!(form(&c, "f"), Form::AtBreak);
}

#[test]
fn the_desks_longest_wait_still_wins_over_a_free_item() {
    // a message from a channel waits at most 30 minutes, whoever asked
    let c = decide(vec![owner("f", Intent::Free, 31 * MINUTE)], true, false, 0);
    assert_eq!(form(&c, "f"), Form::Now);
    let c = decide(vec![owner("f", Intent::Free, 29 * MINUTE)], true, false, 0);
    assert_eq!(form(&c, "f"), Form::AtBreak);
}

#[test]
fn a_free_item_read_by_the_desks_limit_does_not_lift_the_interrupt_limit_for_others() {
    // the hourly limit is spent; a peer item the rule would show now is held to the
    // break. An owner's `next` lifts that limit (as it always did); a free item that the
    // longest-wait rule forces does not.
    let mut p = peer("p", MINUTE);
    p.kind = ItemKind::Calendar;
    p.role = Role::Host;
    p.due = Some(NOW - SECOND);
    let held = decide(
        vec![owner("f", Intent::Free, 31 * MINUTE), p.clone()],
        true,
        false,
        6,
    );
    assert_eq!(form(&held, "f"), Form::Now);
    assert_eq!(form(&held, "p"), Form::AtBreak, "the limit still holds");
    let lifted = decide(vec![owner("n", Intent::Next, MINUTE), p], true, false, 6);
    assert_eq!(form(&lifted, "n"), Form::Now);
    assert_eq!(form(&lifted, "p"), Form::Now, "an owner's next lifts it");
}

#[test]
fn a_peer_is_never_above_an_owner_item_and_a_free_owner_item_is_above_a_peers() {
    // both are read at the same boundary: the peer's item goes where the rule puts it,
    // the free item is read in free time
    let c = decide(
        vec![peer("p", 20 * SECOND), owner("f", Intent::Free, SECOND)],
        false,
        true,
        0,
    );
    assert_eq!(form(&c, "f"), Form::Now);
    assert_eq!(form(&c, "p"), Form::Now);
}
