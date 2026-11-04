//! `spawn` prepares a member a real host starts from, and delivers its brief
//! before it starts; it never overwrites and it refuses what is not a name.

mod common;

use std::time::{Duration, Instant};

use rung_fleet::brief::{Autonomy, Brief};
use rung_fleet::lifecycle::Spawn;
use rung_fleet::spawn::{SpawnError, launch, spawn};
use rung_host::startup;
use rung_testkit::TempDir;

fn what() -> Spawn {
    let brief = Brief::new("a plan", "reviewed", "a report", Autonomy::Ask).unwrap();
    Spawn::new(brief, "crew-1").unwrap()
}

#[test]
fn a_spawned_member_starts_as_an_ordinary_host_and_reads_its_brief() {
    let root = TempDir::new("fleet-spawn");
    let r = root.path();
    let grants = TempDir::new("fleet-grants");
    // SAFETY: the only test in this binary that reads or sets the variable.
    unsafe { std::env::set_var("RUNG_HOME", r.join("rung-home")) };
    let s = spawn(
        r,
        "crew-1",
        "steward-alpha",
        "plan-1",
        &what(),
        "mock",
        common::once(grants.path()),
    )
    .unwrap();
    let handoff = startup::start(startup::registered(
        startup::configure(&s.config, None).expect("the written config is a valid host config"),
    ))
    .expect("the host starts");
    let thread = std::thread::spawn(move || {
        let _ = handoff.run();
    });

    let mut sent = s.task;
    let end = Instant::now() + Duration::from_secs(30);
    let receipt = loop {
        match sent.confirm() {
            Ok(rc) => break rc,
            Err(p) => {
                assert!(Instant::now() < end, "the brief never reached the record");
                sent = p.into_sent();
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    let seq = receipt.seq();
    receipt.discard();
    let mut item = serde_json::Value::Null;
    rung_host::read::scan(&s.state, |l| {
        if l.seq == seq {
            item = l.get("item").clone();
        }
    })
    .unwrap();
    assert_eq!(item["role"], "owner");
    let text = item["text"].as_str().unwrap();
    assert!(
        text.contains("task: plan-1") && text.contains("objective: a plan"),
        "{text}"
    );

    std::fs::write(s.config.parent().unwrap().join("STOP"), "").unwrap();
    thread.join().unwrap();
}

#[test]
fn spawn_refuses_a_bad_name_an_existing_member_and_a_bad_engine() {
    let root = TempDir::new("fleet-spawn-refuse");
    let r = root.path();
    let grants = TempDir::new("fleet-grants");
    for bad in ["Crew 1", "", "../x", "a/b"] {
        assert!(matches!(
            spawn(
                r,
                bad,
                "s",
                "t",
                &what(),
                "mock",
                common::once(grants.path())
            ),
            Err(SpawnError::BadName(_))
        ));
    }
    assert!(matches!(
        spawn(
            r,
            "crew-2",
            "s",
            "t",
            &what(),
            "mo ck",
            common::once(grants.path())
        ),
        Err(SpawnError::BadEngine(_))
    ));
    let first = spawn(
        r,
        "crew-2",
        "s",
        "t",
        &what(),
        "mock",
        common::once(grants.path()),
    )
    .unwrap();
    drop(first);
    let marker = r.join("crew-2/workspace/mine.txt");
    std::fs::write(&marker, "keep").unwrap();
    assert!(matches!(
        spawn(
            r,
            "crew-2",
            "s",
            "t",
            &what(),
            "mock",
            common::once(grants.path())
        ),
        Err(SpawnError::Exists(_))
    ));
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "keep");
}

#[test]
fn launch_runs_the_host_binary_with_the_config() {
    let root = TempDir::new("fleet-spawn-launch");
    let grants = TempDir::new("fleet-grants");
    let s = spawn(
        root.path(),
        "crew-3",
        "s",
        "t",
        &what(),
        "mock",
        common::once(grants.path()),
    )
    .unwrap();
    // `true` ignores its arguments: enough to show the process is started and owned.
    let mut child = launch(std::path::Path::new("/bin/true"), &s.config).unwrap();
    assert!(child.wait().unwrap().success());
    drop(s);
}

#[test]
fn the_command_spawns_a_member_and_refuses_a_bad_name() {
    let root = TempDir::new("fleet-spawn-cli");
    let r = root.path();
    let grants = TempDir::new("fleet-grants");
    let run = |name: &str| {
        std::process::Command::new(env!("CARGO_BIN_EXE_rung-fleet"))
            .args(["spawn", name, "--root"])
            .arg(r)
            .args([
                "--from",
                "chief",
                "--task",
                "t1",
                "--assignee",
                "w",
                "--objective",
                "o",
                "--done-when",
                "d",
                "--delivery",
                "x",
                "--autonomy",
                "ask",
            ])
            .args(common::grant_flags(grants.path(), "spawn"))
            .output()
            .unwrap()
            .status
            .code()
    };
    assert_eq!(run("crew-9"), Some(0));
    assert!(r.join("crew-9/rung-host.yaml").exists());
    assert!(r.join("crew-9/inbox/spawn-crew-9.msg").exists());
    assert_eq!(run("Bad Name"), Some(3));
    assert_eq!(run("crew-9"), Some(3), "never overwrites");
}
