//! `stop` goes through a real host's stop authority: the host records who
//! asked and why, halts, and the member stays in the list.

use std::time::{Duration, Instant};

use rung_fleet::delivery::CorrelationId;
use rung_fleet::steer::stop;
use rung_fleet::verbs;
use rung_host::startup;
use rung_testkit::TempDir;
use serde_json::json;

#[test]
fn stop_records_who_and_why_then_the_member_halts_and_stays_listed() {
    let root = TempDir::new("fleet-stop");
    let r = root.path();
    // The host registers itself under the rung home; keep that out of the user's.
    // SAFETY: this is the only test in the binary, and it sets the variable first.
    unsafe { std::env::set_var("RUNG_HOME", r.join("rung-home")) };
    let d = r.join("steward-alpha");
    std::fs::create_dir_all(d.join("workspace")).unwrap();
    std::fs::create_dir_all(d.join("inbox")).unwrap();
    let cfg = d.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "name: steward-alpha\nstate: {d}/state\nworkspace: {d}/workspace\n\
             inbox: {d}/inbox\nengine:\n  kind: mock\nmemory: false\n",
            d = d.display()
        ),
    )
    .unwrap();
    let handoff = startup::start(startup::registered(
        startup::configure(&cfg, None).expect("a valid config"),
    ))
    .expect("the host starts");
    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let _ = handoff.run();
        let _ = tx.send(());
    });
    let index = r.join("registry");
    std::fs::create_dir_all(&index).unwrap();
    let entry = json!({
        "schema": 1, "id": "steward-alpha", "name": "steward-alpha",
        "state_dir": d.join("state"), "config": cfg, "workspace": d.join("workspace"),
        "address": null, "pid": 1, "started": 1000, "version": "test",
    });
    std::fs::write(index.join("steward-alpha.json"), entry.to_string()).unwrap();

    let id = CorrelationId::new("stop-1").unwrap();
    let mut sent = stop(
        &index,
        "steward-alpha",
        "chief",
        id,
        "the task is withdrawn",
    )
    .unwrap();
    let end = Instant::now() + Duration::from_secs(30);
    let receipt = loop {
        match sent.confirm() {
            Ok(rc) => break rc,
            Err(p) => {
                assert!(Instant::now() < end, "the record never held the stop");
                sent = p.into_sent();
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    let seq = receipt.seq();
    receipt.discard();
    rx.recv_timeout(Duration::from_secs(30))
        .expect("the host halts");
    thread.join().unwrap();

    let mut item = serde_json::Value::Null;
    rung_host::read::scan(&d.join("state"), |l| {
        if l.seq == seq {
            item = l.get("item").clone();
        }
    })
    .unwrap();
    assert_eq!(item["role"], "owner");
    assert_eq!(item["control"], "stop");
    assert_eq!(item["channel"], "fleet:chief", "who asked");
    assert!(
        item["text"]
            .as_str()
            .unwrap()
            .contains("the task is withdrawn"),
        "why"
    );

    let rows = verbs::list(&index).unwrap();
    assert_eq!(rows.len(), 1, "a stopped member stays in the list");
}
