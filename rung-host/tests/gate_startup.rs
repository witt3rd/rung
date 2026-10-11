//! G-r: the startup-handoff ladder. `rung-host run --config FILE` goes
//! Configured → Listed → Recovered → Handed (or Refused). The process half
//! runs the binary against a scripted router on loopback (recorded-shape
//! listing fixtures, scripted completions): a first run, a restart on the
//! same state, refused configurations, and an unreachable router. The
//! compile half pins that no stage can be skipped or forged.

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

use common::*;
use rung_host::gates::{self, Served, StartupRun};
use rung_host::record::{Line, Record};
use rung_host::sim;
use rung_host::sim::http::{LoopbackProvider, Reply, Request};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");
const KEY_ENV: &str = "G_R_ROUTER_KEY";

fn fixture(name: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/openrouter")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn config(state: &Path, base_url: &str, extra: &str) -> String {
    format!(
        "state: {state}\nengine:\n  kind: agent\n  base_url: {base_url}\n  api_key_env: {KEY_ENV}\n  reasoning: medium\nladder:\n  - qwen/qwen3.8-27b:free\n  - nvidia/nemotron-3-super-120b-a12b:free\nmemory: false\n{extra}",
        state = state.display()
    )
}

fn run(cfg: &Path, turns: u64, key: Option<&str>) -> Output {
    let mut c = Command::new(BIN);
    c.args([
        "run",
        "--config",
        cfg.to_str().unwrap(),
        "--turns",
        &turns.to_string(),
    ]);
    // The host registers itself: keep that out of the operator's home.
    c.env("RUNG_HOME", cfg.with_extension("rung-home"));
    match key {
        Some(k) => c.env(KEY_ENV, k),
        None => c.env_remove(KEY_ENV),
    };
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run rung-host")
}

fn lines(state: &Path) -> Vec<Line> {
    Record::read_dir(state.join("record")).unwrap_or_default()
}

#[test]
fn startup_lists_recovers_and_hands_off_or_refuses() {
    sim::test_timeout(900);
    let listing = fixture("models.json");
    let endpoints: BTreeMap<String, Value> =
        serde_json::from_value(fixture("endpoints.json")).unwrap();
    let guard = sim::temp_dir_guard("gate-r");
    let root = guard.path().to_path_buf();
    let state = root.join("state");
    let before_record = Arc::new(Mutex::new(Vec::<bool>::new()));
    let (b2, s2) = (before_record.clone(), state.clone());
    let provider = LoopbackProvider::start(move |r: &Request<'_>| {
        if r.method == "GET" {
            let path = r.path.strip_prefix("/api/v1").unwrap_or(r.path);
            if path == "/models" {
                b2.lock().unwrap().push(!s2.join("record").exists());
                return Reply::json(200, &listing);
            }
            if let Some(id) = path
                .strip_prefix("/models/")
                .and_then(|p| p.strip_suffix("/endpoints"))
            {
                return match endpoints.get(id) {
                    Some(v) => Reply::json(200, v),
                    None => Reply::json(404, &json!({"error": {"message": "no such model"}})),
                };
            }
            return Reply::json(404, &json!({}));
        }
        let model = r.body["model"].as_str().unwrap_or("").to_string();
        let served = Served {
            model,
            prompt: r.body.to_string().len() as u64 / 4,
            cached: 0,
            cache_write: 0,
            completion: 7,
            cost_usd: 0.0,
        };
        Reply::completion("Upstream", Some("nothing pulls right now"), &[], served)
    });
    let mut out = StartupRun::default();

    // A good configuration: a first run, then a restart on the same state.
    let cfg = root.join("rung-host.yaml");
    std::fs::write(&cfg, config(&state, &provider.url, "")).unwrap();
    let first = run(&cfg, 3, Some("test-key-not-a-secret"));
    out.first_exit = first.status.code();
    out.listed_before_record = before_record.lock().unwrap().first() == Some(&true);
    let restart = run(&cfg, 6, Some("test-key-not-a-secret"));
    out.restart_exit = restart.status.code();

    // Refused configurations leave the state alone.
    let refuse = |name: &str, body: String, key: Option<&str>, needle: &str| {
        let st = root.join(format!("refused-{name}"));
        let file = root.join(format!("{name}.yaml"));
        std::fs::write(
            &file,
            body.replace(&state.display().to_string(), &st.display().to_string()),
        )
        .unwrap();
        let o = run(&file, 1, key);
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        (
            o.status.code().unwrap_or(-1),
            err.contains(needle),
            !st.exists(),
        )
    };
    out.refused.insert(
        "bad_field".into(),
        refuse(
            "bad_field",
            config(&state, &provider.url, "ladderz: []\n"),
            Some("k"),
            "ladderz",
        ),
    );
    out.refused.insert(
        "unset_key".into(),
        refuse(
            "unset_key",
            config(&state, &provider.url, ""),
            None,
            KEY_ENV,
        ),
    );

    // An unreachable router: the start goes on, the failure is recorded.
    let dead = root.join("dead");
    let dead_cfg = root.join("dead.yaml");
    std::fs::write(
        &dead_cfg,
        config(&dead, "http://127.0.0.1:1/api/v1", "backoff_base_ms: 50\n"),
    )
    .unwrap();
    let d = run(&dead_cfg, 2, Some("k"));
    out.unreachable_exit = d.status.code();
    out.unreachable_lines = lines(&dead);

    let ls = lines(&state);
    let g = gates::g_r(&ls, &out);
    assert_gate_in(&state, &ls, &g);
    assert!(
        provider.seen().iter().all(|h| h.loopback),
        "a request left loopback"
    );
}

#[test]
fn no_stage_of_the_startup_ladder_can_be_skipped_or_forged() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui_startup/*.rs");
}
