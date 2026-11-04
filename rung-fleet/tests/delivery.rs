//! Delivery against a real host: the receipt names a line of the receiver's
//! record, and a duplicate adds nothing.

use std::path::Path;
use std::time::{Duration, Instant};

use rung_fleet::delivery::{CorrelationId, Delivery, Kind, Sent, Target, accepted_count, send};
use rung_host::startup;
use rung_testkit::TempDir;

const GUARD: Duration = Duration::from_secs(30);

fn start(root: &Path) -> (Target, std::thread::JoinHandle<()>) {
    let dir = root.join("steward-alpha");
    std::fs::create_dir_all(dir.join("inbox")).unwrap();
    std::fs::create_dir_all(dir.join("workspace")).unwrap();
    let cfg = dir.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "name: steward-alpha\nstate: {d}/state\nworkspace: {d}/workspace\n\
             inbox: {d}/inbox\nstop_file: {d}/STOP\nengine:\n  kind: mock\nmemory: false\n",
            d = dir.display()
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
    (
        Target {
            inbox: dir.join("inbox"),
            state: dir.join("state"),
        },
        thread,
    )
}

fn delivery(id: &str) -> Delivery {
    Delivery {
        correlation: CorrelationId::new(id).unwrap(),
        kind: Kind::Completion,
        from: "crew-1".into(),
        text: "the change is done".into(),
    }
}

/// Confirm until the receiver's record holds the item.
fn confirm(mut sent: Sent) -> rung_fleet::delivery::Receipt {
    let end = Instant::now() + GUARD;
    loop {
        match sent.confirm() {
            Ok(r) => return r,
            Err(p) => {
                assert!(Instant::now() < end, "the record never held the item");
                sent = p.into_sent();
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

#[test]
fn the_record_is_the_delivery_and_a_duplicate_adds_nothing() {
    let root = TempDir::new("fleet-delivery");
    unsafe { std::env::set_var("RUNG_HOME", root.path().join("home")) };
    let (target, thread) = start(root.path());

    let d = delivery("done-t-1");
    // Sent twice before the host reads the inbox: one file.
    let first = send(&target, &d).unwrap();
    let again = send(&target, &d).unwrap();
    let receipt = confirm(first);
    assert!(receipt.seq() > 0, "the receipt names a record line");
    assert_eq!(receipt.correlation(), &d.correlation);
    let line_seq = receipt.seq();
    let ack = receipt.record();
    assert_eq!(ack.seq, line_seq);
    confirm(again).discard();

    // Sent again after the host accepted it (and removed the file): a retry
    // or a restart of the sender. The receipt is the same line.
    let late = send(&target, &d).unwrap();
    // A different delivery sent after: once it is in the record, the poll that
    // saw the duplicate has run.
    let marker = send(&target, &delivery("marker")).unwrap();
    confirm(marker).discard();
    assert_eq!(confirm(late).seq(), line_seq);
    assert_eq!(accepted_count(&target.state, &d.correlation).unwrap(), 1);

    std::fs::write(target.state.parent().unwrap().join("STOP"), "").unwrap();
    thread.join().unwrap();
}

#[test]
fn an_unsafe_correlation_id_is_refused() {
    for bad in ["", "../x", "a/b", ".hidden", "a b", "x\0y"] {
        assert!(CorrelationId::new(bad).is_err(), "{bad:?}");
    }
    assert!(CorrelationId::new("done-t-1.v2_a").is_ok());
}

#[test]
fn the_refusals_do_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/receipt_dropped.rs");
    t.compile_fail("tests/ui/receipt_used_twice.rs");
}

#[test]
fn a_damaged_receiver_record_is_not_reported_as_waiting() {
    use rung_fleet::delivery::{DeliveryError, Pending};
    let root = TempDir::new("fleet-damaged");
    let state = root.path().join("state");
    let inbox = root.path().join("inbox");
    std::fs::create_dir_all(state.join("record")).unwrap();
    // A terminated line that is not a record line: damage, not a write in flight.
    std::fs::write(
        state.join("record/seg-000000.ndjson"),
        "not a record line\n",
    )
    .unwrap();
    let target = Target { inbox, state };
    let sent = send(&target, &delivery("damaged-1")).unwrap();
    match sent.confirm() {
        Err(Pending::Unreadable(_, DeliveryError::Record(why))) => {
            assert!(why.contains("damaged"), "the damage is named: {why}");
        }
        other => panic!("a damaged record must surface as unreadable, got {other:?}"),
    }
}
