//! The delegation lifecycle: the happy path runs, and each refusal is a
//! compile-fail case pinned in `tests/ui`.

use rung_fleet::lifecycle::{Completion, Finished, HoldKey, Signal, Spawn, closure, delegation};
use rung_fleet::{Autonomy, Brief, BriefError};

fn brief() -> Brief {
    Brief::new("add a flag", "tests pass", "pull request", Autonomy::Ask).unwrap()
}

fn active() -> delegation::Active {
    let spawned = delegation::Spawned::new(
        Spawn {
            brief: brief(),
            assignee: "worker-1".into(),
        },
        delegation::Carry { task: "t-1".into() },
    );
    delegation::active(delegation::assigned(spawned))
}

#[test]
fn a_task_runs_from_spawned_to_retired() {
    let mut a = active();
    a.payload.signal = Signal::Beat;
    let a = match delegation::step(a) {
        Ok(delegation::StepOutcome::Progress(a)) => a,
        _ => panic!("expected progress"),
    };
    assert_eq!(a.payload.beats, 1);
    let mut a = a;
    a.payload.signal = Signal::Finish(Completion {
        summary: "done".into(),
    });
    let done = match delegation::step(a) {
        Ok(delegation::StepOutcome::Done(d)) => d,
        _ => panic!("expected done"),
    };
    let finishing = closure::Finishing::new(
        Finished::from_done(done),
        closure::Carry { task: "t-1".into() },
    );
    match closure::step(finishing) {
        Ok(closure::StepOutcome::Retired(r)) => {
            assert_eq!(r.into_payload().completion.summary, "done")
        }
        _ => panic!("expected retired"),
    }
}

#[test]
fn a_stalled_task_returns_to_active() {
    let mut a = active();
    a.payload.signal = Signal::Stall;
    let stalled = match delegation::step(a) {
        Ok(delegation::StepOutcome::Stalled(s)) => s,
        _ => panic!("expected stalled"),
    };
    let a = delegation::restart(stalled);
    assert_eq!(a.payload.stalls, 1);
    assert!(a.payload.hold.is_none());
}

#[test]
fn a_hold_stays_open_until_its_own_key_resolves_it() {
    let mut a = active();
    a.payload.signal = Signal::Hold(HoldKey("decide-1".into()));
    let held = match delegation::step(a) {
        Ok(delegation::StepOutcome::Held(h)) => h,
        _ => panic!("expected held"),
    };
    // Resumed with no resolution, or the wrong one: held again.
    let a = delegation::resume(held);
    assert!(a.payload.hold.is_some());
    let held = match delegation::step(a) {
        Ok(delegation::StepOutcome::Held(h)) => h,
        _ => panic!("still held"),
    };
    let mut a = delegation::resume(held);
    a.payload.resolve(HoldKey("other".into()));
    let held = match delegation::step(a) {
        Ok(delegation::StepOutcome::Held(h)) => h,
        _ => panic!("still held"),
    };
    let mut a = delegation::resume(held);
    assert!(a.payload.hold.is_some());
    // Only its own key closes it.
    a.payload.resolve(HoldKey("decide-1".into()));
    let held = match delegation::step(a) {
        Ok(delegation::StepOutcome::Held(h)) => h,
        _ => panic!("held until resumed"),
    };
    let a = delegation::resume(held);
    assert!(a.payload.hold.is_none());
}

#[test]
fn a_brief_refuses_an_empty_field() {
    assert_eq!(
        Brief::new("", "d", "p", Autonomy::Decide),
        Err(BriefError::Empty("objective"))
    );
    assert_eq!(
        Brief::new("o", "  ", "p", Autonomy::Decide),
        Err(BriefError::Empty("done_when"))
    );
}

#[test]
fn the_refusals_do_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/skip_assignment.rs");
    t.compile_fail("tests/ui/forge_active.rs");
    t.compile_fail("tests/ui/forge_finished.rs");
    t.compile_fail("tests/ui/brief_missing_field.rs");
    t.compile_fail("tests/ui/autonomy_has_no_default.rs");
}
