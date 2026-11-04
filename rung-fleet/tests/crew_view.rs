//! Crew lines as a view over the peer items a real host records: the fleet
//! reports facts into a member's inbox, the host records them as it records any
//! item, and `verbs::crew` reads them back in order. The host is unchanged.

mod common;

use std::time::{Duration, Instant};

use rung_fleet::brief::{Autonomy, Brief};
use rung_fleet::delivery::CorrelationId;
use rung_fleet::records::CrewRecord;
use rung_fleet::{steer, verbs};
use rung_host::startup;
use rung_testkit::TempDir;
use serde_json::json;

fn confirmed(mut sent: rung_fleet::delivery::Sent) -> u64 {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        match sent.confirm() {
            Ok(r) => {
                let n = r.seq();
                r.discard();
                return n;
            }
            Err(p) => {
                assert!(Instant::now() < end, "the record never held the fact");
                sent = p.into_sent();
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

#[test]
fn crew_facts_reported_to_a_host_read_back_as_crew_lines() {
    let root = TempDir::new("fleet-crew-view");
    let r = root.path();
    let grants = TempDir::new("fleet-grants");
    // SAFETY: the only test in this binary; sets the variable before any host starts.
    unsafe { std::env::set_var("RUNG_HOME", r.join("rung-home")) };
    let d = r.join("steward-alpha");
    std::fs::create_dir_all(d.join("workspace")).unwrap();
    std::fs::create_dir_all(d.join("inbox")).unwrap();
    let cfg = d.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "name: steward-alpha\nstate: {d}/state\nworkspace: {d}/workspace\n\
             inbox: {d}/inbox\nstop_file: {d}/STOP\nengine:\n  kind: mock\nmemory: false\n",
            d = d.display()
        ),
    )
    .unwrap();
    let handoff = startup::start(startup::registered(
        startup::configure(&cfg, None).expect("a valid config"),
    ))
    .expect("the host starts");
    let thread = std::thread::spawn(move || {
        let _ = handoff.run();
    });
    let index = r.join("registry");
    std::fs::create_dir_all(&index).unwrap();
    let entry = json!({
        "schema": 1, "id": "steward-alpha", "name": "steward-alpha",
        "state_dir": d.join("state"), "config": cfg, "workspace": d.join("workspace"),
        "address": null, "pid": 1, "started": 1000, "version": "test",
    });
    std::fs::write(index.join("steward-alpha.json"), entry.to_string()).unwrap();

    let brief = Brief::new("a plan", "reviewed", "a report", Autonomy::Ask).unwrap();
    let facts = vec![
        CrewRecord::Spawned {
            crew: "c1".into(),
            brief,
            assignee: "worker".into(),
        },
        CrewRecord::Active { crew: "c1".into() },
        CrewRecord::Held {
            crew: "c1".into(),
            key: "k".into(),
        },
    ];
    let mut seqs = Vec::new();
    for (i, f) in facts.iter().enumerate() {
        let id = CorrelationId::new(format!("f-{i}")).unwrap();
        let sent = steer::report(&index, "steward-alpha", "chief", id, f).unwrap();
        seqs.push(confirmed(sent));
    }
    // An owner steer that merely mentions the tag is not a fact (only peer items are).
    let noise = CorrelationId::new("noise").unwrap();
    let sent = steer::steer(
        &index,
        "steward-alpha",
        "chief",
        noise,
        "[fleet:crew x]",
        common::once(grants.path()),
    )
    .unwrap();
    confirmed(sent);

    let got = verbs::crew(&index, "steward-alpha").unwrap();
    assert_eq!(got.len(), 3, "{got:?}");
    for (((seq, rec), want), line) in got.iter().zip(&facts).zip(&seqs) {
        assert_eq!(rec, want);
        assert_eq!(seq, line, "the view carries the record line's own number");
    }

    std::fs::write(d.join("STOP"), "").unwrap();
    thread.join().unwrap();
}
