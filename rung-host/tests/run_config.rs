//! `rung-host run --config` for a bounded live run: a `*.msg` inbox, seeded
//! owner calendar entries, a stop file, a run limit, and the decision desk
//! in Shadow mode on a System One decider — with its spend cap and kill
//! file. Loopback only: one scripted router answers the listing, the
//! completions and `/systemone`. No key, $0.

mod common;

use std::path::Path;
use std::process::{Command, Output, Stdio};

use rung_host::gates::Served;
use rung_host::record::{Line, Record};
use rung_host::sim;
use rung_host::sim::http::{LoopbackProvider, Reply, Request};
use serde_json::{Map, Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");
const KEY_ENV: &str = "RUN_CONFIG_ROUTER_KEY";

fn run(cfg: &Path, turns: Option<u64>, key: Option<&str>) -> Output {
    let mut c = Command::new(BIN);
    c.args(["run", "--config", cfg.to_str().unwrap()]);
    if let Some(t) = turns {
        c.args(["--turns", &t.to_string()]);
    }
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

/// Answer every asked question: a Noul at 0.9, a Choice on its first option.
fn systemone(body: &Value) -> Value {
    let mut answers = Map::new();
    for (id, q) in body["questions"].as_object().cloned().unwrap_or_default() {
        let a = match q["type"].as_str() {
            Some("noul") => json!({"type": "noul", "noul": 0.9}),
            _ => {
                let opts: Vec<String> = q["criteria"]
                    .as_object()
                    .map(|o| o.keys().cloned().collect())
                    .unwrap_or_default();
                let probs: Map<String, Value> = opts
                    .iter()
                    .enumerate()
                    .map(|(i, o)| (o.clone(), json!(if i == 0 { 1.0 } else { 0.0 })))
                    .collect();
                json!({"type": "choice", "choice": opts[0], "confidence": 0.8, "probabilities": probs})
            }
        };
        answers.insert(id, a);
    }
    json!({"model": "typesafe/jev-1.13", "answers": answers,
           "usage": {"input_tokens": 100, "cost": 0.0000042}})
}

fn router() -> LoopbackProvider {
    LoopbackProvider::start(move |r: &Request<'_>| {
        if r.path.ends_with("/systemone") {
            return Reply::json(200, &systemone(r.body));
        }
        if r.method == "GET" {
            return Reply::json(404, &json!({}));
        }
        let served = Served {
            model: r.body["model"].as_str().unwrap_or("").to_string(),
            prompt: r.body.to_string().len() as u64 / 4,
            cached: 0,
            cache_write: 0,
            completion: 5,
            cost_usd: 0.0,
        };
        Reply::completion("Upstream", Some("noted"), &[], served)
    })
}

fn config(root: &Path, url: &str, desk: &str) -> String {
    let d = root.display();
    format!(
        "state: {d}/state\nworkspace: {d}/sandbox\ninbox: {d}/inbox\nstop_file: {d}/STOP\n\
         engine:\n  kind: agent\n  base_url: {url}\n  api_key_env: {KEY_ENV}\n\
         ladder:\n  - qwen/qwen3.8-27b:free\nlisting: false\nmemory: false\n\
         calendar:\n  - {{id: tea, in_s: 0, text: Tea break, firm: true}}\n{desk}"
    )
}

fn desk(root: &Path, url: &str, cap: f64) -> String {
    format!(
        "desk:\n  mode: shadow\n  decider: jev\n  base_url: {url}\n  api_key_env: {KEY_ENV}\n  \
         cap_usd_day: {cap}\n  kill_file: {}/DESK_OFF\n",
        root.display()
    )
}

#[test]
fn a_bounded_live_shaped_run_shadows_the_desk_and_honours_its_switches() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("run-config");
    let root = guard.path().to_path_buf();
    let provider = router();
    let cfg = root.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        config(&root, &provider.url, &desk(&root, &provider.url, 0.25)),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("inbox")).unwrap();
    std::fs::write(
        root.join("inbox/m1.msg"),
        r#"{"role":"owner","text":"Hello, what are you working on?"}"#,
    )
    .unwrap();
    let o = run(&cfg, Some(3), Some("not-a-secret"));
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let ls = lines(&root.join("state"));
    let kinds = |k: &'static str| ls.iter().filter(move |l| l.kind == k);
    let start = kinds("host.start").next().expect("host.start");
    assert_eq!(start.get("desk_mode"), &json!("shadow"));
    assert_eq!(start.str("desk"), "jev");
    assert!(
        kinds("calendar.added").any(|l| l.get("entry")["id"] == "tea"),
        "seeded calendar entry missing"
    );
    assert!(
        kinds("stimulus.accepted").any(|l| l.get("item")["id"] == "m1"),
        "the inbox message was not accepted"
    );
    let asks: Vec<&Line> = kinds("desk.ask").collect();
    assert!(!asks.is_empty());
    assert!(asks.iter().any(|l| l.str("outcome") == "answered"));
    for l in ls.iter().filter(|l| l.kind.starts_with("decision.")) {
        assert!(
            l.get("by").get("rule").is_some(),
            "shadow decided by jev: {:?}",
            l.body
        );
        if l.get("by")["rule"] == "shadow" {
            assert!(l.get("agree").is_boolean(), "no agree on {:?}", l.body);
        }
    }
    // The key never reaches the record.
    let text: String = ls
        .iter()
        .map(|l| Value::Object(l.body.clone()).to_string())
        .collect();
    assert!(!text.contains("not-a-secret"));

    // The kill switch: the desk stops asking at once.
    std::fs::write(root.join("DESK_OFF"), "").unwrap();
    let before = lines(&root.join("state")).len();
    let o = run(&cfg, Some(5), Some("not-a-secret"));
    assert_eq!(o.status.code(), Some(0));
    let after = lines(&root.join("state"));
    let new_asks: Vec<&Line> = after[before..]
        .iter()
        .filter(|l| l.kind == "desk.ask")
        .collect();
    assert!(!new_asks.is_empty());
    assert!(
        new_asks
            .iter()
            .all(|l| l.str("outcome") == "killed" || l.str("outcome") == "nothing_to_ask"),
        "an ask went out with the kill file present"
    );
    std::fs::remove_file(root.join("DESK_OFF")).unwrap();

    // The stop file halts the host.
    std::fs::write(root.join("STOP"), "").unwrap();
    let o = run(&cfg, None, Some("not-a-secret"));
    assert_eq!(o.status.code(), Some(0));
    assert!(
        lines(&root.join("state"))
            .iter()
            .any(|l| l.kind == "halted")
    );
    std::fs::remove_file(root.join("STOP")).unwrap();
}

#[test]
fn a_zero_cap_keeps_every_ask_home() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("run-config-cap");
    let root = guard.path().to_path_buf();
    let provider = router();
    let cfg = root.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        config(&root, &provider.url, &desk(&root, &provider.url, 0.0)),
    )
    .unwrap();
    let o = run(&cfg, Some(2), Some("k"));
    assert_eq!(o.status.code(), Some(0));
    let ls = lines(&root.join("state"));
    assert!(
        ls.iter()
            .filter(|l| l.kind == "desk.ask")
            .all(|l| l.str("outcome") != "answered")
    );
    assert!(
        provider
            .seen()
            .iter()
            .all(|s| !s.path.ends_with("/systemone"))
    );
}

#[test]
fn a_bad_desk_or_an_unset_desk_key_is_refused() {
    let guard = sim::temp_dir_guard("run-config-refused");
    let root = guard.path().to_path_buf();
    let url = "http://127.0.0.1:1/api/v1";
    for (name, desk, needle) in [
        (
            "mode",
            "desk:\n  mode: sometimes\n".to_string(),
            "desk.mode",
        ),
        (
            "decider",
            "desk:\n  mode: shadow\n".to_string(),
            "desk.decider",
        ),
        (
            "key",
            "desk:\n  mode: shadow\n  decider: jev\n  api_key_env: RUN_CONFIG_UNSET_DESK_KEY\n"
                .to_string(),
            "RUN_CONFIG_UNSET_DESK_KEY",
        ),
        (
            "cap",
            format!("{}  cap_usd_ask: -1\n", desk(&root, url, 0.25)),
            "desk.cap_usd_ask",
        ),
    ] {
        let cfg = root.join(format!("{name}.yaml"));
        std::fs::write(&cfg, config(&root, url, &desk)).unwrap();
        let o = run(&cfg, Some(1), Some("k"));
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(2), "{name}: {err}");
        assert!(err.contains(needle), "{name}: {err}");
        assert!(!root.join("state").exists(), "{name} touched the state");
    }
}

#[test]
fn a_rung_the_router_will_not_route_for_this_account_is_stepped_past() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("run-config-unroutable");
    let root = guard.path().to_path_buf();
    let provider = LoopbackProvider::start(move |r: &Request<'_>| {
        let model = r.body["model"].as_str().unwrap_or("").to_string();
        if model == "a/closed:free" {
            return Reply::json(
                404,
                &json!({"error": {"code": 404,
                    "message": "0 endpoints out of 1 requested are available matching your guardrail restrictions and data policy.",
                    "metadata": {"ineligibility_reasons": [{"reason": "zdr-violation-by-guardrail", "endpoint_count": 1}]}}}),
            );
        }
        let served = Served {
            model,
            prompt: 10,
            cached: 0,
            cache_write: 0,
            completion: 5,
            cost_usd: 0.0,
        };
        Reply::completion("Upstream", Some("noted"), &[], served)
    });
    let d = root.display();
    let cfg = root.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "state: {d}/state\nengine:\n  kind: agent\n  base_url: {}\n  api_key_env: {KEY_ENV}\n\
             ladder:\n  - a/closed:free\n  - b/open:free\nlisting: false\nmemory: false\nbackoff_base_ms: 20\n",
            provider.url
        ),
    )
    .unwrap();
    let o = run(&cfg, Some(3), Some("k"));
    assert_eq!(o.status.code(), Some(0));
    let ls = lines(&root.join("state"));
    let failed = ls
        .iter()
        .find(|l| l.kind == "turn.ended" && l.str("status") == "failed")
        .expect("the closed rung's turn failed");
    assert_eq!(
        failed.get("failure")["unroutable"],
        json!(["zdr-violation-by-guardrail"])
    );
    let down = ls
        .iter()
        .find(|l| l.kind == "model.switch")
        .expect("a step down");
    assert_eq!(down.str("direction"), "down");
    assert_eq!(down.str("to"), "b/open:free");
    assert!(ls.iter().any(|l| l.kind == "turn.ended"
        && l.str("status") == "completed"
        && l.str("model") == "b/open:free"));
}

#[test]
fn a_run_limit_ends_a_long_backoff() {
    sim::test_timeout(120);
    let guard = sim::temp_dir_guard("run-config-limit");
    let root = guard.path().to_path_buf();
    // Every call is refused; the host's backoff starts at ten minutes.
    let provider =
        LoopbackProvider::start(move |_r: &Request<'_>| Reply::provider_429("Upstream", 10));
    let d = root.display();
    let cfg = root.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "state: {d}/state\nengine:\n  kind: agent\n  base_url: {}\n  api_key_env: {KEY_ENV}\n\
             ladder:\n  - a/one:free\nlisting: false\nmemory: false\nrun_for_s: 3\nbackoff_base_ms: 600000\n",
            provider.url
        ),
    )
    .unwrap();
    let t = std::time::Instant::now();
    let o = run(&cfg, None, Some("k"));
    assert_eq!(o.status.code(), Some(0));
    assert!(
        t.elapsed() < std::time::Duration::from_secs(30),
        "the backoff outlived the run limit: {:?}",
        t.elapsed()
    );
    let ls = lines(&root.join("state"));
    assert!(ls.iter().any(|l| l.kind == "degraded"));
    assert_eq!(
        ls.iter().find(|l| l.kind == "halted").unwrap().get("why")["by"],
        json!("limit")
    );
}
