#![allow(dead_code)]
//! Shared by the tests that start the real `rung-host` binary.

use std::path::PathBuf;
use std::process::{Child, Command};

/// Kills the child if the test fails before it stopped on its own.
pub struct Guard(pub Child);
impl Drop for Guard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// Build `rung-host` and return the executable cargo reports.
pub fn host_binary() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["build", "--locked", "-p", "rung-host", "--bin", "rung-host"])
        .args(["--message-format=json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["reason"] == "compiler-artifact" && v["target"]["name"] == "rung-host")
        .filter_map(|v| v["executable"].as_str().map(PathBuf::from))
        .next_back()
        .expect("cargo reported the rung-host executable")
}

/// A state directory whose record holds the given items (id, role, text) as
/// `stimulus.accepted` lines: the principal's channel for a test that needs a
/// grant without a running host. A role of `owner` is the principal's.
#[allow(dead_code)]
pub fn record_with(state: &std::path::Path, items: &[(&str, &str, &str)]) {
    std::fs::create_dir_all(state.join("record")).unwrap();
    let mut out = String::new();
    for (i, (id, role, text)) in items.iter().enumerate() {
        let line = rung_host::record::Line {
            seq: i as u64 + 1,
            at: 1000 + i as i64,
            kind: "stimulus.accepted".into(),
            body: serde_json::json!({"item": {"id": id, "role": role, "text": text}})
                .as_object()
                .unwrap()
                .clone(),
        };
        out.push_str(&line.text());
        out.push('\n');
    }
    std::fs::write(state.join("record/seg-000000.ndjson"), out).unwrap();
}

static GRANTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn granted(
    scratch: &std::path::Path,
    act: &str,
) -> (rung_fleet::authority::Granted, std::path::PathBuf) {
    let n = GRANTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let id = format!("grant-{n}");
    let dir = scratch.join(format!("principal-{n}"));
    record_with(
        &dir,
        &[(
            &id,
            "owner",
            &rung_fleet::authority::Granted::text(act, None),
        )],
    );
    (
        rung_fleet::authority::Granted::from_record(&dir, &id).unwrap(),
        scratch.join("grant-ledger"),
    )
}

/// A one-time token for act `A`, as the principal's owner item would make it.
#[allow(dead_code)]
pub fn once<A: rung_fleet::authority::Act>(
    scratch: &std::path::Path,
) -> rung_fleet::authority::Once<A> {
    let (g, ledger) = granted(scratch, A::WORD);
    g.once(&ledger).unwrap()
}

/// The command's authority flags for a fresh one-time grant of `act`.
pub fn grant_flags(scratch: &std::path::Path, act: &str) -> Vec<String> {
    let n = GRANTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let id = format!("grant-{n}");
    let dir = scratch.join(format!("principal-{n}"));
    record_with(
        &dir,
        &[(
            &id,
            "owner",
            &rung_fleet::authority::Granted::text(act, None),
        )],
    );
    vec![
        "--grant-state".into(),
        dir.display().to_string(),
        "--grant".into(),
        id,
        "--ledger".into(),
        scratch.join("grant-ledger").display().to_string(),
    ]
}
