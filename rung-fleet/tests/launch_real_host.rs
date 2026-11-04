//! `launch` starts the real `rung-host` binary (the other spawn tests use a
//! stand-in): the member registers itself, reads its brief, and stops at its
//! stop file. The instance index is a scratch directory, never the user's.

mod common;

use common::{Guard, host_binary};
use std::time::{Duration, Instant};

use rung_fleet::brief::{Autonomy, Brief};
use rung_fleet::lifecycle::Spawn;
use rung_fleet::spawn::{launch, spawn};
use rung_host::registry;
use rung_testkit::TempDir;

#[test]
fn a_launched_member_is_the_real_host_and_reads_its_brief() {
    let bin = host_binary();
    let root = TempDir::new("fleet-launch-real");
    let r = root.path();
    let grants = TempDir::new("fleet-grants");
    let home = r.join("rung-home");
    // SAFETY: the only test in this binary; the child inherits the variable.
    unsafe { std::env::set_var("RUNG_HOME", &home) };

    let brief = Brief::new("a plan", "reviewed", "a report", Autonomy::Ask).unwrap();
    let what = Spawn::new(brief, "crew-1").unwrap();
    let s = spawn(
        r,
        "crew-1",
        "steward-alpha",
        "plan-1",
        &what,
        "mock",
        common::once(grants.path()),
    )
    .unwrap();
    let mut child = Guard(launch(&bin, &s.config).expect("the real host starts"));

    let mut sent = s.task;
    let end = Instant::now() + Duration::from_secs(60);
    let receipt = loop {
        match sent.confirm() {
            Ok(rc) => break rc,
            Err(p) => {
                assert!(Instant::now() < end, "the brief never reached the record");
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "the host exited early"
                );
                sent = p.into_sent();
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    let seq = receipt.seq();
    receipt.discard();
    let mut text = String::new();
    rung_host::read::scan(&s.state, |l| {
        if l.seq == seq {
            text = l.get("item")["text"].as_str().unwrap_or("").to_string();
        }
    })
    .unwrap();
    assert!(
        text.contains("task: plan-1") && text.contains("objective: a plan"),
        "{text}"
    );

    let rows = registry::list_in(&home.join("instances")).unwrap();
    assert!(
        rows.iter().any(|row| row.id == "crew-1"),
        "the member is in the index"
    );

    std::fs::write(r.join("crew-1/STOP"), "").unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    while child.0.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < end,
            "the host did not stop at its stop file"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(home.exists(), "the index lives under the scratch home");
}
