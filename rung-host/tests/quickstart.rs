//! The quickstart (docs/rung-host-quickstart.md) runs as written: the
//! example config, a dropped `*.msg`, the record, the stop file. Mock
//! engine; no key, no network.

use std::path::Path;
use std::process::{Command, Stdio};

use rung_host::record::{Line, Record};
use rung_host::sim;

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");

fn run(cwd: &Path, turns: Option<&str>) -> std::process::ExitStatus {
    let mut c = Command::new(BIN);
    c.current_dir(cwd)
        .env("RUNG_HOME", cwd.join("rung-home"))
        .args(["run", "--config", "rung-host-mock.yaml"]);
    if let Some(t) = turns {
        c.args(["--turns", t]);
    }
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run rung-host")
}

fn lines(cwd: &Path) -> Vec<Line> {
    Record::read_dir(cwd.join("demo/state/record")).unwrap_or_default()
}

#[test]
fn the_quickstart_example_runs() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("quickstart");
    let cwd = guard.path();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/rung-host-mock.yaml");
    std::fs::copy(example, cwd.join("rung-host-mock.yaml")).unwrap();
    std::fs::create_dir_all(cwd.join("demo/inbox")).unwrap();
    std::fs::create_dir_all(cwd.join("demo/sandbox")).unwrap();
    std::fs::write(
        cwd.join("demo/inbox/hello.msg"),
        r#"{"role":"owner","text":"Hello, what are you working on?"}"#,
    )
    .unwrap();

    assert_eq!(run(cwd, Some("3")).code(), Some(0));
    let ls = lines(cwd);
    assert!(
        ls.iter()
            .any(|l| l.kind == "stimulus.accepted" && l.get("item")["id"] == "hello")
    );
    assert!(
        ls.iter()
            .any(|l| l.kind == "stimulus.disposed" && l.str("disposition") == "answered")
    );
    assert!(ls.iter().any(|l| l.kind == "outbox.queued"));

    std::fs::write(cwd.join("demo/STOP"), "").unwrap();
    assert_eq!(run(cwd, None).code(), Some(0));
    assert!(lines(cwd).iter().any(|l| l.kind == "halted"));
}
