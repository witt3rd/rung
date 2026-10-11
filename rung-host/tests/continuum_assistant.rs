//! The Continuum research assistant config (examples/continuum) parses and
//! runs on the mock engine: only the engine block is swapped. No key, no network.

use std::path::Path;
use std::process::{Command, Stdio};

use rung_host::record::Record;
use rung_host::sim;

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");

#[test]
fn the_continuum_assistant_config_runs_on_the_mock_engine() {
    sim::test_timeout(120);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/continuum/continuum-assistant.yaml");
    let text = std::fs::read_to_string(path).unwrap();
    // Config shape the commission requires.
    assert!(text.contains("identity:") && text.contains("read-only"));
    assert!(text.contains("stealth/space-bunny-alpha"));
    assert!(!text.contains("openrouter/free"));
    let engine = "engine:\n  kind: agent\n  base_url: https://openrouter.ai/api/v1\n  api_key_env: OPENROUTER_API_KEY\n";
    assert!(text.contains(engine));
    let text = text.replace(engine, "engine:\n  kind: mock\n");

    let guard = sim::temp_dir_guard("continuum-assistant");
    let cwd = guard.path();
    std::fs::write(cwd.join("c.yaml"), text).unwrap();
    std::fs::create_dir_all(cwd.join("data/inbox")).unwrap();
    std::fs::create_dir_all(cwd.join("data/workspace/continuum")).unwrap();
    std::fs::write(
        cwd.join("data/inbox/q.msg"),
        r#"{"role":"owner","text":"What should we run next?"}"#,
    )
    .unwrap();
    let st = Command::new(BIN)
        .current_dir(cwd)
        .env("RUNG_HOME", cwd.join("rung-home"))
        .args(["run", "--config", "c.yaml", "--turns", "3"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(st.code(), Some(0));
    let ls = Record::read_dir(cwd.join("data/state/record")).unwrap_or_default();
    assert!(ls.iter().any(|l| l.kind == "stimulus.accepted" && l.get("item")["id"] == "q"));
    assert!(ls.iter().any(|l| l.kind == "stimulus.disposed"));
}
