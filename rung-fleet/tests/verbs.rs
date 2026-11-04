//! `list` and `read` against an instance index and records on disk: they
//! answer from the host's own readers and change nothing under any directory
//! they touch (`read-and-list-never-write`).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use rung_fleet::brief::{Autonomy, Brief};
use rung_fleet::records::CrewRecord;
use rung_fleet::verbs::{self, VerbError};
use rung_host::record::Line;
use rung_testkit::TempDir;
use serde_json::json;

/// Register a member by hand (an entry file) with a record of `lines`.
fn member(root: &Path, id: &str, lines: &[Line]) {
    let state = root.join(id).join("state");
    let rec = state.join("record");
    std::fs::create_dir_all(&rec).unwrap();
    let text: String = lines.iter().map(|l| format!("{}\n", l.text())).collect();
    std::fs::write(rec.join("seg-000000.ndjson"), text).unwrap();
    let reg = root.join("registry");
    std::fs::create_dir_all(&reg).unwrap();
    let entry = json!({
        "schema": 1, "id": id, "name": id, "state_dir": state, "config": null,
        "workspace": root.join(id).join("workspace"), "address": null,
        "pid": 1, "started": 1000, "version": "test",
    });
    std::fs::write(reg.join(format!("{id}.json")), entry.to_string()).unwrap();
}

fn plain(seq: u64, kind: &str) -> Line {
    Line::parse(&format!(r#"{{"seq":{seq},"at":1,"kind":"{kind}"}}"#)).unwrap()
}

fn world(root: &Path) {
    member(
        root,
        "chief",
        &[plain(1, "host.start"), plain(2, "epoch.rollover")],
    );
    let brief = Brief::new("a plan", "reviewed", "a report", Autonomy::Ask).unwrap();
    member(
        root,
        "steward-alpha",
        &[
            plain(1, "host.start"),
            CrewRecord::Spawned {
                crew: "c1".into(),
                brief,
                assignee: "w".into(),
            }
            .to_line(2, 5),
            Line::parse(r#"{"seq":3,"at":6,"kind":"crew.teleported","crew":"c1"}"#).unwrap(),
            CrewRecord::Active { crew: "c1".into() }.to_line(4, 7),
        ],
    );
}

/// Every file under `root` with its bytes and modification time.
fn snapshot(root: &Path) -> BTreeMap<String, (Vec<u8>, std::time::SystemTime)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let m = std::fs::metadata(&p).unwrap().modified().unwrap();
                out.insert(p.display().to_string(), (std::fs::read(&p).unwrap(), m));
            }
        }
    }
    out
}

#[test]
fn list_and_read_answer_from_the_index_and_the_record() {
    let root = TempDir::new("fleet-verbs");
    world(root.path());
    let reg = root.path().join("registry");
    let rows = verbs::list(&reg).unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        ["chief", "steward-alpha"],
        "a member that is not running stays listed"
    );

    let r = verbs::read(&reg, "steward-alpha", 1, 2).unwrap();
    let seqs: Vec<u64> = r.lines.iter().map(|l| l.seq).collect();
    assert_eq!(seqs, [2, 3]);
    assert_eq!(r.total, 4);
    assert!(r.damaged.is_none());

    let crew = verbs::crew(&reg, "steward-alpha").unwrap();
    let kinds: Vec<(u64, &str)> = crew.iter().map(|(s, r)| (*s, r.kind())).collect();
    assert_eq!(
        kinds,
        [(2, "crew.spawned"), (4, "crew.active")],
        "the unknown kind is skipped"
    );

    assert!(matches!(
        verbs::read(&reg, "nobody", 0, 1),
        Err(VerbError::NoSuchMember(_))
    ));
    assert!(matches!(
        verbs::read(&reg, "../chief", 0, 1),
        Err(VerbError::NoSuchMember(_))
    ));
}

#[test]
fn list_and_read_change_nothing() {
    let root = TempDir::new("fleet-verbs-ro");
    world(root.path());
    let reg = root.path().join("registry");
    let before = snapshot(root.path());
    let rows = verbs::list(&reg).unwrap();
    let _ = rung_host::registry::rows_json(&rows);
    verbs::read(&reg, "chief", 0, 10).unwrap();
    verbs::crew(&reg, "steward-alpha").unwrap();
    assert!(
        before == snapshot(root.path()),
        "a verb wrote under the tree"
    );
}

#[test]
fn the_command_prints_what_the_library_returns() {
    let root = TempDir::new("fleet-verbs-cli");
    world(root.path());
    let reg = root.path().join("registry");
    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_rung-fleet"))
            .arg("--registry")
            .arg(&reg)
            .args(args)
            .output()
            .unwrap();
        (out.status.code(), String::from_utf8(out.stdout).unwrap())
    };
    let before = snapshot(root.path());
    let (code, json) = run(&["list", "--json"]);
    assert_eq!(code, Some(0));
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["instances"].as_array().unwrap().len(), 2);
    let (_, crew) = run(&["read", "steward-alpha", "--crew"]);
    assert_eq!(crew, "2\tcrew.spawned\tc1\n4\tcrew.active\tc1\n");
    let (_, lines) = run(&["read", "chief", "--after", "1"]);
    assert_eq!(lines.lines().count(), 1);
    assert_eq!(run(&["read", "nobody"]).0, Some(1));
    assert_eq!(run(&["frobnicate"]).0, Some(2));
    assert!(
        before == snapshot(root.path()),
        "the command wrote under the tree"
    );
}
