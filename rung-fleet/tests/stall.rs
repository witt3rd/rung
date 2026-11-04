//! `a-stalled-crew-is-escalated-not-looped`, on a fake clock: a crew silent
//! past its bound is stalled by the ladder, retried once, and the same task
//! stalling again is escalated to the steward's parent instead of retried.

use rung_fleet::delivery::{Kind, Target, send};
use rung_fleet::lifecycle::{Spawn, delegation};
use rung_fleet::supervise::{Handled, StallError, Watch, handle_stall, tick};
use rung_fleet::tree::Line;
use rung_fleet::{Autonomy, Brief};
use rung_testkit::TempDir;

fn active(task: &str) -> delegation::Active {
    let brief = Brief::new("o", "d", "r", Autonomy::Ask).unwrap();
    let spawned = delegation::Spawned::new(
        Spawn::new(brief, "crew-1").unwrap(),
        delegation::Carry { task: task.into() },
    );
    delegation::active(delegation::assigned(spawned))
}

fn line() -> Line {
    let mut l = Line::new();
    l.attach("chief-main", "steward-alpha").unwrap();
    l.attach("steward-alpha", "crew-1").unwrap();
    l
}

fn stalled(out: delegation::StepOutcome) -> delegation::Stalled {
    match out {
        delegation::StepOutcome::Stalled(s) => s,
        _ => panic!("expected stalled"),
    }
}

#[test]
fn silence_past_the_bound_is_retried_once_then_escalated_up_one_level() {
    let root = TempDir::new("fleet-stall");
    let line = line();
    let mut clock = 1_000;
    let mut watch = Watch::new(100, clock);
    let mut task = active("t-1");

    // Within the bound: the task goes on.
    clock += 60;
    task = match tick(task, &watch, clock).ok().unwrap() {
        delegation::StepOutcome::Progress(a) => a,
        _ => panic!("silent too soon"),
    };
    assert!(!watch.silent(clock));

    // Past the bound: stalled, by the ladder. The first stall is retried.
    clock += 200;
    assert!(watch.silent(clock));
    let first = stalled(tick(task, &watch, clock).ok().unwrap());
    assert_eq!(first.source().payload.stalls, 1);
    task = match handle_stall(first, "steward-alpha", &line).ok().unwrap() {
        Handled::Retried(a) => a,
        Handled::Escalated(_) => panic!("the first stall is retried"),
    };
    watch.beat(clock);

    // Silent again: the same task stalls a second time and is not retried.
    clock += 500;
    let second = stalled(tick(task, &watch, clock).ok().unwrap());
    let esc = match handle_stall(second, "steward-alpha", &line).ok().unwrap() {
        Handled::Escalated(e) => e,
        Handled::Retried(_) => panic!("a second stall is escalated, not retried"),
    };
    assert_eq!(esc.to, "chief-main", "up one level, not to the principal");
    assert_eq!(esc.delivery.kind, Kind::Escalation);
    assert_eq!(esc.delivery.from, "steward-alpha");
    assert_eq!(esc.work.stalls, 2);

    // The escalation is an item in the parent's inbox, and sending it again
    // adds nothing.
    let target = Target {
        inbox: root.path().join("chief-main/inbox"),
        state: root.path().join("chief-main/state"),
    };
    drop(send(&target, &esc.delivery).unwrap());
    drop(send(&target, &esc.delivery).unwrap());
    let sent: Vec<_> = std::fs::read_dir(&target.inbox)
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(sent.len(), 1);
    let body = std::fs::read_to_string(sent[0].path()).unwrap();
    assert!(
        body.contains("fleet:escalation from=steward-alpha"),
        "{body}"
    );
    assert!(body.contains("t-1"), "{body}");
}

#[test]
fn a_steward_with_no_parent_cannot_escalate() {
    let mut task = active("t-2");
    let watch = Watch::new(10, 0);
    let mut line = Line::new();
    line.attach("steward-alpha", "crew-1").unwrap(); // a steward not attached to a chief
    for _ in 0..2 {
        let s = stalled(tick(task, &watch, 1_000).ok().unwrap());
        match handle_stall(s, "steward-alpha", &line) {
            Ok(Handled::Retried(a)) => task = a,
            Err(StallError::NoParent(who)) => {
                assert_eq!(who, "steward-alpha");
                return;
            }
            _ => panic!("unexpected"),
        }
    }
    panic!("the second stall must be refused with no parent");
}
