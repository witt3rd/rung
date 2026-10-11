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

#[test]
fn a_rung_refused_for_the_account_is_not_stepped_onto_again() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("run-config-refused-once");
    let root = guard.path().to_path_buf();
    let provider = LoopbackProvider::start(move |r: &Request<'_>| {
        let model = r.body["model"].as_str().unwrap_or("").to_string();
        let turn = rung_host::gates::request_turn(r.body).unwrap_or(0);
        if model == "c/closed:free" {
            return Reply::json(
                404,
                &json!({"error": {"code": 404,
                    "message": "0 endpoints out of 1 requested are available matching your guardrail restrictions and data policy.",
                    "metadata": {"ineligibility_reasons": [{"reason": "zdr-violation-by-guardrail", "endpoint_count": 1}]}}}),
            );
        }
        // The one rung that routes is rate-limited upstream on two turns.
        if turn == 2 || turn == 4 {
            return Reply::provider_429("Upstream", 1);
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
             ladder:\n  - b/open:free\n  - c/closed:free\nlisting: false\nmemory: false\nbackoff_base_ms: 20\n",
            provider.url
        ),
    )
    .unwrap();
    let o = run(&cfg, Some(7), Some("k"));
    assert_eq!(o.status.code(), Some(0));
    let ls = lines(&root.join("state"));
    let on_closed = ls
        .iter()
        .filter(|l| l.kind == "turn.started" && l.str("model") == "c/closed:free")
        .count();
    assert_eq!(on_closed, 1, "the refused rung was tried again");
    let refused = ls
        .iter()
        .find(|l| l.kind == "ladder.refused")
        .expect("the refusal is on record");
    assert_eq!(
        refused.get("reasons"),
        &json!(["zdr-violation-by-guardrail"])
    );
    let back = ls
        .iter()
        .find(|l| l.kind == "model.switch" && l.str("why").starts_with("refused"))
        .expect("a switch off the refused rung");
    assert_eq!(back.str("to"), "b/open:free");
    // The second rate limit finds nothing standing below: it stays put.
    assert!(
        !ls.iter()
            .filter(|l| l.seq > refused.seq)
            .any(|l| l.kind == "model.switch" && l.str("to") == "c/closed:free"),
        "a step-down landed on the refused rung"
    );
}

#[test]
fn an_owner_waiting_through_a_provider_backoff_hears_from_the_host_at_once() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("run-config-owner-ack");
    let root = guard.path().to_path_buf();
    let provider = LoopbackProvider::start(move |r: &Request<'_>| {
        let turn = rung_host::gates::request_turn(r.body).unwrap_or(0);
        // The provider is rate-limited for the first two turns.
        if turn <= 2 {
            return Reply::provider_429("Upstream", 1);
        }
        let served = Served {
            model: r.body["model"].as_str().unwrap_or("").to_string(),
            prompt: 10,
            cached: 0,
            cache_write: 0,
            completion: 5,
            cost_usd: 0.0,
        };
        Reply::completion("Upstream", Some("Here is my answer."), &[], served)
    });
    let d = root.display();
    std::fs::create_dir_all(root.join("inbox")).unwrap();
    std::fs::write(
        root.join("inbox/m1.msg"),
        r#"{"role":"owner","text":"Are you there?"}"#,
    )
    .unwrap();
    let cfg = root.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "state: {d}/state\ninbox: {d}/inbox\nengine:\n  kind: agent\n  base_url: {}\n  api_key_env: {KEY_ENV}\n\
             ladder:\n  - a/one:free\nlisting: false\nmemory: false\nbackoff_base_ms: 600\n",
            provider.url
        ),
    )
    .unwrap();
    let o = run(&cfg, Some(4), Some("k"));
    assert_eq!(o.status.code(), Some(0));
    let ls = lines(&root.join("state"));
    let acks: Vec<&Line> = ls
        .iter()
        .filter(|l| l.kind == "outbox.queued" && l.str("source") == "host:ack")
        .collect();
    assert_eq!(
        acks.len(),
        1,
        "one acknowledgement per owner item, across every wait"
    );
    let ack = acks[0];
    assert_eq!(ack.str("item"), "m1");
    assert_eq!(ack.str("channel"), "owner");
    assert!(ack.str("text").contains("no model was asked"));
    // It came during the first backoff, before any turn could answer.
    let first_wait = ls.iter().find(|l| l.kind == "degraded").unwrap();
    let wait_end = ls.iter().find(|l| l.kind == "degraded.ended").unwrap();
    assert!(first_wait.seq < ack.seq && ack.seq < wait_end.seq);
    let answered = ls
        .iter()
        .find(|l| l.kind == "stimulus.disposed" && l.str("id") == "m1")
        .expect("the owner item was disposed");
    assert_eq!(answered.str("disposition"), "answered");
    assert!(ack.seq < answered.seq);
}

#[test]
fn a_pacing_wait_sends_no_acknowledgement() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("run-config-no-ack");
    let root = guard.path().to_path_buf();
    let provider = LoopbackProvider::start(move |r: &Request<'_>| {
        let served = Served {
            model: r.body["model"].as_str().unwrap_or("").to_string(),
            prompt: 10,
            cached: 0,
            cache_write: 0,
            completion: 5,
            cost_usd: 0.0,
        };
        Reply::completion("Upstream", Some("Here is my answer."), &[], served)
    });
    let d = root.display();
    std::fs::create_dir_all(root.join("inbox")).unwrap();
    let cfg = root.join("rung-host.yaml");
    // One request a minute: after the first free turn the host waits on
    // pacing, a wait an owner item may cut.
    std::fs::write(
        &cfg,
        format!(
            "state: {d}/state\ninbox: {d}/inbox\nengine:\n  kind: agent\n  base_url: {}\n  api_key_env: {KEY_ENV}\n\
             ladder:\n  - a/one:free\nlisting: false\nmemory: false\nquota:\n  rpd: 1000\n  rpm: 1\n",
            provider.url
        ),
    )
    .unwrap();
    let mut child = Command::new(BIN)
        .args(["run", "--config", cfg.to_str().unwrap(), "--turns", "2"])
        .env(KEY_ENV, "k")
        .env("RUNG_HOME", root.join("rung-home"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("run rung-host");
    // The owner writes while the host is in its pacing wait.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !lines(&root.join("state"))
        .iter()
        .any(|l| l.kind == "degraded" && l.str("class") == "paced")
    {
        assert!(
            std::time::Instant::now() < deadline,
            "the run never waited on pacing"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    std::fs::write(
        root.join("inbox/m1.msg"),
        r#"{"role":"owner","text":"Are you there?"}"#,
    )
    .unwrap();
    assert!(child.wait().unwrap().success());
    let ls = lines(&root.join("state"));
    let wait = ls
        .iter()
        .find(|l| l.kind == "degraded" && l.str("class") == "paced")
        .unwrap();
    let ended = ls
        .iter()
        .find(|l| l.kind == "degraded.ended" && l.seq > wait.seq)
        .unwrap();
    let accepted = ls
        .iter()
        .find(|l| l.kind == "stimulus.accepted" && l.get("item")["id"] == "m1")
        .expect("the owner item was accepted");
    assert!(
        wait.seq < accepted.seq && accepted.seq < ended.seq,
        "the owner item did not arrive during the pacing wait"
    );
    assert!(
        !ls.iter()
            .any(|l| l.kind == "outbox.queued" && l.str("source") == "host:ack"),
        "an acknowledgement went out during a pacing wait"
    );
}

fn openrouter_fixture(name: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/openrouter")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// A router that lists from the recorded fixtures, refuses `refused` for
/// the account on every request, and rate-limits turn 2 of the rest.
fn listing_router(refused: &'static str) -> LoopbackProvider {
    let listing = openrouter_fixture("models.json");
    let endpoints: std::collections::BTreeMap<String, Value> =
        serde_json::from_value(openrouter_fixture("endpoints.json")).unwrap();
    LoopbackProvider::start(move |r: &Request<'_>| {
        if r.method == "GET" {
            let path = r.path.strip_prefix("/api/v1").unwrap_or(r.path);
            if path == "/models" {
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
        if model == refused {
            return Reply::json(
                404,
                &json!({"error": {"code": 404,
                    "message": "0 endpoints out of 1 requested are available matching your guardrail restrictions and data policy.",
                    "metadata": {"ineligibility_reasons": [{"reason": "zdr-violation-by-guardrail", "endpoint_count": 1}]}}}),
            );
        }
        if rung_host::gates::request_turn(r.body) == Some(2) {
            return Reply::provider_429("Upstream", 1);
        }
        let served = Served {
            model,
            prompt: 10,
            cached: 0,
            cache_write: 0,
            completion: 1,
            cost_usd: 0.0,
        };
        Reply::completion("Upstream", Some("OK"), &[], served)
    })
}

fn probe_config(root: &Path, url: &str, extra: &str) -> std::path::PathBuf {
    let d = root.display();
    let cfg = root.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "state: {d}/state\nengine:\n  kind: agent\n  base_url: {url}\n  api_key_env: {KEY_ENV}\n\
             ladder:\n  - qwen/qwen3.8-27b:free\n  - nvidia/nemotron-3-super-120b-a12b:free\n\
             memory: false\nbackoff_base_ms: 20\n{extra}"
        ),
    )
    .unwrap();
    cfg
}

#[test]
fn a_keyed_probe_at_start_finds_a_refused_rung_before_any_turn() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("run-config-probe");
    let root = guard.path().to_path_buf();
    let refused = "nvidia/nemotron-3-super-120b-a12b:free";
    let provider = listing_router(refused);
    let cfg = probe_config(&root, &provider.url, "");
    let o = run(&cfg, Some(4), Some("k"));
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let ls = lines(&root.join("state"));
    let first_turn = ls.iter().find(|l| l.kind == "turn.started").unwrap();
    let probed = ls
        .iter()
        .find(|l| l.kind == "ladder.probed")
        .expect("the ladder was probed");
    assert!(probed.seq < first_turn.seq, "probed after the first turn");
    let verdicts: Vec<(String, String)> = probed
        .get("rungs")
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["model"].as_str().unwrap().to_string(),
                r["verdict"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        verdicts,
        vec![
            ("qwen/qwen3.8-27b:free".to_string(), "routes".to_string()),
            (refused.to_string(), "refused".to_string())
        ]
    );
    let r = ls
        .iter()
        .find(|l| l.kind == "ladder.refused")
        .expect("the refusal is on record");
    assert_eq!(r.str("by"), "probe");
    assert!(r.seq < first_turn.seq);
    // The rate limit on turn 2 finds nothing standing below: no turn ever
    // lands on the refused rung.
    assert!(
        ls.iter()
            .any(|l| l.kind == "turn.ended" && l.str("status") == "failed")
    );
    assert!(
        !ls.iter()
            .any(|l| l.kind == "turn.started" && l.str("model") == refused),
        "a turn ran on the rung the probe found refused"
    );
    // The probes carried the key; the listing did not.
    let seen = provider.seen();
    let probes: Vec<_> = seen
        .iter()
        .filter(|h| {
            h.method == "POST" && h.body["max_tokens"] == 1 && h.body.get("tools").is_none()
        })
        .collect();
    assert_eq!(probes.len(), 2, "one probe per standing rung");
    assert!(probes.iter().all(|h| h.auth));
    assert!(seen.iter().filter(|h| h.method == "GET").all(|h| !h.auth));
}

#[test]
fn probes_can_be_turned_off_and_ride_only_with_the_listing() {
    sim::test_timeout(300);
    for (name, extra) in [
        ("off", "probe: false\n"),
        ("no-listing", "listing: false\n"),
    ] {
        let guard = sim::temp_dir_guard(&format!("run-config-probe-{name}"));
        let root = guard.path().to_path_buf();
        let provider = listing_router("nvidia/nemotron-3-super-120b-a12b:free");
        let cfg = probe_config(&root, &provider.url, extra);
        let o = run(&cfg, Some(1), Some("k"));
        assert_eq!(o.status.code(), Some(0), "{name}");
        let ls = lines(&root.join("state"));
        assert!(
            !ls.iter().any(|l| l.kind == "ladder.probed"),
            "{name}: probed"
        );
        assert!(
            provider.seen().iter().all(|h| h.body["max_tokens"] != 1),
            "{name}: a probe request went out"
        );
    }
}

#[test]
fn probes_pass_the_governor_and_a_held_probe_is_on_record() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("run-config-probe-paced");
    let root = guard.path().to_path_buf();
    let provider = listing_router("nvidia/nemotron-3-super-120b-a12b:free");
    // One request a minute: the first probe spends it, the second is held.
    let cfg = probe_config(
        &root,
        &provider.url,
        "quota:\n  rpd: 1000\n  rpm: 1\nrun_for_s: 2\n",
    );
    let o = run(&cfg, None, Some("k"));
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let ls = lines(&root.join("state"));
    let probed = ls
        .iter()
        .find(|l| l.kind == "ladder.probed")
        .expect("the ladder was probed");
    let verdicts: Vec<String> = probed
        .get("rungs")
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["verdict"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(verdicts, vec!["routes".to_string(), "skipped".to_string()]);
    assert!(
        probed.get("rungs")[1]["why"]
            .as_str()
            .unwrap()
            .starts_with("paced")
    );
    assert_eq!(probed.u64("probes"), 1);
    let sent = provider
        .seen()
        .iter()
        .filter(|h| h.method == "POST" && h.body["max_tokens"] == 1)
        .count();
    assert_eq!(sent, 1, "a probe went out past the pacer");
    // The probe is in the pacer's window: the first turn waits on it.
    assert!(
        ls.iter()
            .any(|l| l.kind == "degraded" && l.str("class") == "paced"),
        "the probe did not count against the minute"
    );
}

#[test]
fn a_probe_that_refuses_the_current_rung_names_itself_in_the_switch() {
    sim::test_timeout(300);
    let guard = sim::temp_dir_guard("run-config-probe-current");
    let root = guard.path().to_path_buf();
    let refused = "nvidia/nemotron-3-super-120b-a12b:free";
    let provider = listing_router(refused);
    let d = root.display();
    let cfg = root.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "state: {d}/state\nengine:\n  kind: agent\n  base_url: {}\n  api_key_env: {KEY_ENV}\n\
             ladder:\n  - {refused}\n  - qwen/qwen3.8-27b:free\nmemory: false\n",
            provider.url
        ),
    )
    .unwrap();
    let o = run(&cfg, Some(1), Some("k"));
    assert_eq!(o.status.code(), Some(0));
    let ls = lines(&root.join("state"));
    let first_turn = ls.iter().find(|l| l.kind == "turn.started").unwrap();
    let switch = ls
        .iter()
        .find(|l| l.kind == "model.switch")
        .expect("a switch off the refused rung");
    assert!(switch.seq < first_turn.seq);
    assert!(
        switch
            .str("why")
            .starts_with("refused (probe): zdr-violation-by-guardrail"),
        "{}",
        switch.str("why")
    );
    assert_eq!(first_turn.str("model"), "qwen/qwen3.8-27b:free");
}
