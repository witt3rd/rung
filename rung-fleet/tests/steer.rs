//! `steer` puts one owner-class item in the target's inbox and nothing else;
//! against a real host the receipt names the record line that holds it.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

use rung_fleet::delivery::CorrelationId;
use rung_fleet::steer::{SteerError, steer};
use rung_host::startup;
use rung_testkit::TempDir;
use serde_json::json;

/// Register `id` by hand: an entry naming its config and state directory.
fn register(root: &Path, id: &str, config: &Path, state: &Path) {
    let reg = root.join("registry");
    std::fs::create_dir_all(&reg).unwrap();
    let entry = json!({
        "schema": 1, "id": id, "name": id, "state_dir": state, "config": config,
        "workspace": root.join(id).join("workspace"), "address": null,
        "pid": 1, "started": 1000, "version": "test",
    });
    std::fs::write(reg.join(format!("{id}.json")), entry.to_string()).unwrap();
}

fn write_config(root: &Path, id: &str, inbox_line: &str) -> std::path::PathBuf {
    let d = root.join(id);
    std::fs::create_dir_all(d.join("workspace")).unwrap();
    let cfg = d.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "name: {id}\nstate: {d}/state\nworkspace: {d}/workspace\n{inbox_line}\n\
             stop_file: {d}/STOP\nengine:\n  kind: mock\nmemory: false\n",
            d = d.display()
        ),
    )
    .unwrap();
    cfg
}

fn files(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                out.insert(e.path().display().to_string());
            }
        }
    }
    out
}

fn id(s: &str) -> CorrelationId {
    CorrelationId::new(s).unwrap()
}

#[test]
fn a_steer_is_one_owner_item_in_the_inbox_and_nothing_else() {
    let root = TempDir::new("fleet-steer");
    let r = root.path();
    let inbox = r.join("steward-alpha/inbox");
    let cfg = write_config(r, "steward-alpha", &format!("inbox: {}", inbox.display()));
    register(r, "steward-alpha", &cfg, &r.join("steward-alpha/state"));
    let before = files(r);

    let sent = steer(
        &r.join("registry"),
        "steward-alpha",
        "chief",
        id("s-1"),
        "go left",
    )
    .unwrap();
    drop(sent);

    let added: Vec<String> = files(r).difference(&before).cloned().collect();
    assert_eq!(added, [inbox.join("s-1.msg").display().to_string()]);
    let body: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(inbox.join("s-1.msg")).unwrap()).unwrap();
    assert_eq!(body["role"], "owner");
    let text = body["text"].as_str().unwrap();
    assert!(
        text.starts_with("[fleet:steer from=chief id=s-1]"),
        "{text}"
    );
    assert!(text.ends_with("go left"));
}

#[test]
fn a_steer_that_cannot_find_an_inbox_says_why() {
    let root = TempDir::new("fleet-steer-refuse");
    let r = root.path();
    let reg = r.join("registry");
    assert!(matches!(
        steer(&reg, "nobody", "chief", id("s"), "x"),
        Err(SteerError::NoSuchMember(_))
    ));
    let none = write_config(r, "no-inbox", "");
    register(r, "no-inbox", &none, &r.join("no-inbox/state"));
    assert!(matches!(
        steer(&reg, "no-inbox", "chief", id("s"), "x"),
        Err(SteerError::NoInbox(_))
    ));
    let rel = write_config(r, "rel-inbox", "inbox: inbox");
    register(r, "rel-inbox", &rel, &r.join("rel-inbox/state"));
    assert!(matches!(
        steer(&reg, "rel-inbox", "chief", id("s"), "x"),
        Err(SteerError::NoInbox(_))
    ));
}

#[test]
fn a_steer_reaches_a_real_host_and_the_receipt_names_the_line() {
    let root = TempDir::new("fleet-steer-host");
    let r = root.path();
    let cfg = write_config(
        r,
        "steward-alpha",
        &format!("inbox: {}", r.join("steward-alpha/inbox").display()),
    );
    std::fs::create_dir_all(r.join("steward-alpha/inbox")).unwrap();
    // The host registers itself under the rung home; keep that out of the user's.
    // SAFETY: this is the only test in the binary that reads or sets the variable.
    unsafe { std::env::set_var("RUNG_HOME", r.join("rung-home")) };
    let handoff = startup::start(startup::registered(
        startup::configure(&cfg, None).expect("a valid config"),
    ))
    .expect("the host starts");
    let thread = std::thread::spawn(move || {
        let _ = handoff.run();
    });
    register(r, "steward-alpha", &cfg, &r.join("steward-alpha/state"));

    let mut sent = steer(
        &r.join("registry"),
        "steward-alpha",
        "chief",
        id("s-2"),
        "go",
    )
    .unwrap();
    let end = Instant::now() + Duration::from_secs(30);
    let receipt = loop {
        match sent.confirm() {
            Ok(rc) => break rc,
            Err(p) => {
                assert!(Instant::now() < end, "the record never held the steer");
                sent = p.into_sent();
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    let mut role = String::new();
    rung_host::read::scan(&r.join("steward-alpha/state"), |l| {
        if l.seq == receipt.seq() {
            role = l.get("item")["role"].as_str().unwrap_or("").to_string();
        }
    })
    .unwrap();
    assert_eq!(role, "owner");
    receipt.discard();

    std::fs::write(r.join("steward-alpha/STOP"), "").unwrap();
    thread.join().unwrap();
}
