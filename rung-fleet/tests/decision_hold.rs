//! `a-decision-is-a-held-item`: a decision opens a keyed hold; only a
//! resolution carrying the key closes it.

use rung_fleet::brief::{Autonomy, Brief};
use rung_fleet::records::{CrewRecord as R, open_holds};

fn held(crew: &str, key: &str) -> R {
    R::Held {
        crew: crew.into(),
        key: key.into(),
    }
}
fn resolved(crew: &str, key: &str) -> R {
    R::Resolved {
        crew: crew.into(),
        key: key.into(),
    }
}
fn keys(lines: &[(u64, R)]) -> Vec<String> {
    open_holds(lines).into_iter().map(|h| h.key).collect()
}

#[test]
fn only_a_resolution_with_the_key_closes_a_hold() {
    let brief = Brief::new("o", "d", "r", Autonomy::Ask).unwrap();
    let mut lines = vec![(1, held("t1", "k1"))];
    assert_eq!(keys(&lines), ["k1"]);

    // Later lines that merely answer it, or concern the same task: still open.
    for (n, later) in [
        R::Active { crew: "t1".into() },
        R::Done {
            crew: "t1".into(),
            summary: "decided in passing".into(),
            validator: "v".into(),
        },
        R::Spawned {
            crew: "t1".into(),
            brief,
            assignee: "w".into(),
        },
        R::Retired { crew: "t1".into() },
        resolved("t1", "other-key"),
        resolved("t2", "k1"),
    ]
    .into_iter()
    .enumerate()
    {
        lines.push((2 + n as u64, later));
        assert_eq!(keys(&lines), ["k1"], "line {n} closed the hold");
    }

    // Its own key, for its own task, closes it; saying so twice changes nothing.
    lines.push((20, resolved("t1", "k1")));
    assert!(keys(&lines).is_empty());
    lines.push((21, resolved("t1", "k1")));
    assert!(keys(&lines).is_empty());
}

#[test]
fn holds_are_independent_and_a_resolution_with_no_hold_opens_nothing() {
    let lines = vec![
        (1, resolved("t1", "early")),
        (2, held("t1", "a")),
        (3, held("t1", "b")),
        (4, held("t1", "a")),
        (5, resolved("t1", "a")),
    ];
    let open = open_holds(&lines);
    assert_eq!(open.len(), 1);
    assert_eq!((open[0].key.as_str(), open[0].seq), ("b", 3));
    // A resolution that came before its hold does not pre-close it.
    let lines = vec![(1, resolved("t1", "z")), (2, held("t1", "z"))];
    assert_eq!(keys(&lines), ["z"]);
}
