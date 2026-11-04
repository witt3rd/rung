//! `a-kill-loses-no-completion`: a steward (the real `rung-host` binary) is
//! killed with SIGKILL; the completions its crew sent are in its record once
//! each after it restarts, whether the host had read them before the kill or
//! the crew wrote them while it was down.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{Guard, host_binary};
use rung_fleet::delivery::{CorrelationId, Delivery, Kind, Sent, Target, accepted_count, send};
use rung_fleet::spawn::{launch, spawn};
use rung_fleet::{brief::Autonomy, brief::Brief, lifecycle::Spawn};
use rung_testkit::TempDir;

fn completion(id: &str) -> Delivery {
    Delivery {
        correlation: CorrelationId::new(id).unwrap(),
        kind: Kind::Completion,
        from: "crew-1".into(),
        text: format!("{id}: the change landed"),
    }
}

fn confirm(mut sent: Sent) -> u64 {
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        match sent.confirm() {
            Ok(r) => {
                let seq = r.seq();
                r.discard();
                return seq;
            }
            Err(p) => {
                assert!(Instant::now() < end, "the record never held the item");
                sent = p.into_sent();
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn start(bin: &Path, config: &Path) -> Guard {
    Guard(launch(bin, config).expect("the real host starts"))
}

#[test]
fn a_killed_steward_recovers_every_completion_once() {
    let bin = host_binary();
    let root = TempDir::new("fleet-kill");
    let r = root.path();
    let grants = TempDir::new("fleet-grants");
    // SAFETY: the only test in this binary; children inherit the variable.
    unsafe { std::env::set_var("RUNG_HOME", r.join("rung-home")) };

    let brief = Brief::new("a plan", "reviewed", "a report", Autonomy::Ask).unwrap();
    let what = Spawn::new(brief, "crew-1").unwrap();
    let s = spawn(
        r,
        "steward-alpha",
        "chief-main",
        "plan-1",
        &what,
        "mock",
        common::once(grants.path()),
    )
    .unwrap();
    let target = Target {
        inbox: s.inbox.clone(),
        state: s.state.clone(),
    };
    let mut steward = start(&bin, &s.config);
    confirm(s.task);

    // One completion the steward read before it died.
    let read_before = confirm(send(&target, &completion("done-a")).unwrap());

    // Killed (SIGKILL: no shutdown path runs), with a second completion just
    // written and a third written while it is down.
    let racing = send(&target, &completion("done-b")).unwrap();
    steward.0.kill().unwrap();
    steward.0.wait().unwrap();
    let while_down = send(&target, &completion("done-c")).unwrap();

    // Restart from the same configuration: the record is the recovery.
    let steward = start(&bin, &s.config);
    confirm(racing);
    confirm(while_down);
    // A sender that retries (it cannot know the kill happened) adds nothing,
    // and the line the steward read before the kill is the same line.
    let retried = send(&target, &completion("done-a")).unwrap();
    assert_eq!(confirm(retried), read_before);
    let marker = send(&target, &completion("marker")).unwrap();
    confirm(marker);
    for id in ["done-a", "done-b", "done-c"] {
        let id = CorrelationId::new(id).unwrap();
        assert_eq!(accepted_count(&target.state, &id).unwrap(), 1, "{id:?}");
    }

    std::fs::write(r.join("steward-alpha/STOP"), "").unwrap();
    let mut steward = steward;
    let end = Instant::now() + Duration::from_secs(60);
    while steward.0.try_wait().unwrap().is_none() {
        assert!(Instant::now() < end, "the restarted host did not stop");
        std::thread::sleep(Duration::from_millis(100));
    }
}
