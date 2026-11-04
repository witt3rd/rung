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
