//! `a-queue-has-one-writer`: another member's item reaches a host only as an
//! inbox file, and a second host cannot be started on a state directory a
//! live host holds. Real `rung-host` binary, scratch instance index.

mod common;

use std::time::{Duration, Instant};

use common::{Guard, host_binary};
use rung_fleet::brief::{Autonomy, Brief};
use rung_fleet::delivery::{CorrelationId, Delivery, Kind, Target, send};
use rung_fleet::lifecycle::Spawn;
use rung_fleet::spawn::{launch, spawn};
use rung_testkit::TempDir;

fn count_starts(state: &std::path::Path) -> usize {
    let mut n = 0;
    let _ = rung_host::read::scan(state, |l| {
        if l.kind == "host.start" {
            n += 1;
        }
    });
    n
}

#[test]
fn items_arrive_only_as_inbox_files_and_a_second_host_is_refused() {
    let bin = host_binary();
    let root = TempDir::new("fleet-one-writer");
    let r = root.path();
    // SAFETY: the only test in this binary; children inherit the variable.
    unsafe { std::env::set_var("RUNG_HOME", r.join("rung-home")) };

    let brief = Brief::new("o", "d", "r", Autonomy::Ask).unwrap();
    let what = Spawn::new(brief, "crew-1").unwrap();
    let s = spawn(r, "steward-alpha", "chief-main", "t1", &what, "mock").unwrap();
    let target = Target {
        inbox: s.inbox.clone(),
        state: s.state.clone(),
    };

    // With no host running, a delivery writes the inbox and nothing of the
    // receiver's state: the sender has no door into the queue.
    let other = send(
        &target,
        &Delivery {
            correlation: CorrelationId::new("from-a-peer").unwrap(),
            kind: Kind::Handoff,
            from: "steward-beta".into(),
            text: "an item".into(),
        },
    )
    .unwrap();
    assert!(
        !s.state.exists(),
        "a delivery wrote into the receiver's state"
    );
    assert!(s.inbox.join("from-a-peer.msg").exists());
    drop((other, s.task));

    // The host is the writer: once it runs, the items are in its record.
    let mut host = Guard(launch(&bin, &s.config).expect("the host starts"));
    let end = Instant::now() + Duration::from_secs(60);
    while count_starts(&s.state) == 0 {
        assert!(Instant::now() < end, "the host never started");
        std::thread::sleep(Duration::from_millis(100));
    }
    while s.inbox.join("from-a-peer.msg").exists() {
        assert!(Instant::now() < end, "the host never read the inbox");
        std::thread::sleep(Duration::from_millis(100));
    }

    // A second host on the same state directory is refused and writes nothing.
    let dir2 = r.join("second");
    std::fs::create_dir_all(dir2.join("inbox")).unwrap();
    std::fs::create_dir_all(dir2.join("workspace")).unwrap();
    let config2 = dir2.join("rung-host.yaml");
    std::fs::write(
        &config2,
        format!(
            "name: steward-second\nstate: {}\nworkspace: {d}/workspace\ninbox: {d}/inbox\n\
             stop_file: {d}/STOP\nengine:\n  kind: mock\nmemory: false\n",
            s.state.display(),
            d = dir2.display()
        ),
    )
    .unwrap();
    let mut second = Guard(launch(&bin, &config2).unwrap());
    let end = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(st) = second.0.try_wait().unwrap() {
            break st;
        }
        assert!(Instant::now() < end, "the second host was not refused");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(
        !status.success(),
        "a second host ran on a held state directory"
    );
    assert_eq!(
        count_starts(&s.state),
        1,
        "the refused host wrote to the record"
    );
    assert!(
        host.0.try_wait().unwrap().is_none(),
        "the first host survived"
    );

    std::fs::write(r.join("steward-alpha/STOP"), "").unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    while host.0.try_wait().unwrap().is_none() {
        assert!(Instant::now() < end, "the host did not stop");
        std::thread::sleep(Duration::from_millis(100));
    }
}
