//! The delegation lifecycle: the happy path runs, and each refusal is a
//! compile-fail case pinned in `tests/ui`.

mod common;

use rung_fleet::authority::act;
use rung_fleet::landing::Landed;
use rung_fleet::lifecycle::{Completion, Finished, HoldKey, Signal, Spawn, closure, delegation};
use rung_fleet::{Autonomy, Brief, BriefError};

fn brief() -> Brief {
    Brief::new("add a flag", "tests pass", "pull request", Autonomy::Ask).unwrap()
}

fn active() -> delegation::Active {
    let spawned = delegation::Spawned::new(
        Spawn::new(brief(), "worker-1").unwrap(),
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
    let grants = rung_testkit::TempDir::new("fleet-grants");
    let landed = Landed::scratch(common::once::<act::Discard>(grants.path()));
    let finishing = closure::Finishing::new(
        Finished::from_done(done, landed),
        closure::Carry { task: "t-1".into() },
    );
    match closure::step(finishing) {
        Ok(closure::StepOutcome::Retired(r)) => {
            let r = r.into_payload();
            assert_eq!(r.completion.summary, "done");
            assert!(r.landed.starts_with("scratch, discarded under grant"));
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

/// The claim's proof: both recoverable states return to *active* through
/// their own paired edge, and a hold stays open until its own key arrives.
#[test]
fn held_and_stalled_are_recoverable() {
    // stalled -> restart -> active, and the stall is counted, not lost
    let mut a = active();
    a.payload.signal = Signal::Stall;
    let stalled = match delegation::step(a) {
        Ok(delegation::StepOutcome::Stalled(s)) => s,
        _ => panic!("expected stalled"),
    };
    let a = delegation::restart(stalled);
    assert_eq!(a.payload.stalls, 1);
    // a restarted task works on: a beat is progress, not another stall
    assert!(matches!(
        delegation::step(a),
        Ok(delegation::StepOutcome::Progress(_))
    ));

    // held -> resume -> active only with the hold's own key
    let mut a = active();
    a.payload.signal = Signal::Hold(HoldKey("k".into()));
    let held = match delegation::step(a) {
        Ok(delegation::StepOutcome::Held(h)) => h,
        _ => panic!("expected held"),
    };
    let mut a = delegation::resume(held);
    assert!(a.payload.hold.is_some(), "no resolution: still held");
    a.payload.resolve(HoldKey("k".into()));
    let held = match delegation::step(a) {
        Ok(delegation::StepOutcome::Held(h)) => h,
        _ => panic!("held until resumed"),
    };
    let a = delegation::resume(held);
    assert!(a.payload.hold.is_none());
    assert!(matches!(
        delegation::step(a),
        Ok(delegation::StepOutcome::Progress(_))
    ));
}

#[test]
fn a_brief_refuses_an_empty_field() {
    assert_eq!(
        Brief::new("o", "d", "  ", Autonomy::Ask),
        Err(BriefError::Empty("delivery"))
    );
    assert_eq!(
        Spawn::new(brief(), " ").unwrap_err(),
        BriefError::Empty("assignee")
    );
    assert_eq!(brief().delivery(), "pull request");
    assert_eq!(brief().autonomy(), Autonomy::Ask);

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
    t.compile_fail("tests/ui/brief_literal.rs");
    t.compile_fail("tests/ui/spawn_literal.rs");
    t.compile_fail("tests/ui/held_dropped.rs");
    t.compile_fail("tests/ui/retire_without_landed.rs");
    t.compile_fail("tests/ui/landed_forged.rs");
    t.compile_fail("tests/ui/scratch_with_standing.rs");
    t.compile_fail("tests/ui/autonomy_has_no_default.rs");
}
