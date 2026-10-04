//! G-n: the real engine adapter. The host runs `rung-agent-core`'s engine
//! through `rung_host::adapter` against a scripted provider on loopback that
//! answers in the router's documented wire shape. No live model, no key.

mod common;

use std::sync::Arc;

use common::*;
use rung_host::adapter::{AdapterConfig, AgentEngine};
use rung_host::clock::{Clock, MINUTE, SECOND, SimClock};
use rung_host::gates::{self, Served, request_turn};
use rung_host::sim::http::{LoopbackProvider, Reply, Request};
use rung_host::sim::{self, SIM_START};
use serde_json::{Value, json};

const LADDER: [&str; 3] = ["test/alpha:free", "test/beta:free", "test/gamma:free"];
/// The turn whose first call the upstream of the top rung refuses.
const PROVIDER_429_TURN: u64 = 6;
/// The turn whose first call the router itself refuses.
const PLATFORM_429_TURN: u64 = 12;

/// What the model does in a turn, by turn number and step (tool results so
/// far in the turn).
fn script(turn: u64, step: usize) -> (Option<String>, Vec<(&'static str, Value)>) {
    let done = |t: &str| (Some(t.to_string()), Vec::new());
    match (turn, step) {
        // A disabled tool (free time has no workspace_write).
        (1, 0) => (
            None,
            vec![("ws_write", json!({"path": "x.txt", "text": "x"}))],
        ),
        // A long result: the file holds more than the host returns.
        (2, 0) => (None, vec![("ws_read", json!({"path": "long.txt"}))]),
        (3, 0) => (
            None,
            vec![(
                "note",
                json!({"text": "carried: the long file is in the workspace"}),
            )],
        ),
        // Every step a tool call, until the loop withdraws the tools; no
        // two alike in a row (that is a doom loop, stopped early).
        (5, s) if s < 5 && s % 2 == 0 => (None, vec![("ws_list", json!({"path": ""}))]),
        (5, s) if s < 5 => (None, vec![("ws_read", json!({"path": "long.txt"}))]),
        (5, _) => done("five listings; nothing new"),
        (t, 0) if t % 4 == 0 => (
            None,
            vec![(
                "trace",
                json!({"what_pulled": format!("the listing at turn {t}"), "where_it_went": "nowhere yet"}),
            )],
        ),
        (_, 0) => done("nothing pulls right now"),
        (_, _) => done("done for this turn"),
    }
}

/// Tool results since the newest turn header.
fn step_of(body: &Value) -> usize {
    let msgs = body["messages"].as_array().cloned().unwrap_or_default();
    let header = msgs
        .iter()
        .rposition(|m| {
            m["role"] == "user"
                && match &m["content"] {
                    Value::String(t) => t.starts_with("[turn "),
                    Value::Array(p) => p
                        .first()
                        .and_then(|x| x["text"].as_str())
                        .is_some_and(|t| t.starts_with("[turn ")),
                    _ => false,
                }
        })
        .unwrap_or(0);
    msgs[header..]
        .iter()
        .filter(|m| m["role"] == "assistant")
        .count()
}

#[test]
fn the_engine_adapter_runs_the_host_against_a_loopback_provider() {
    sim::test_timeout(600);
    let clock = Arc::new(SimClock::new(SIM_START));
    let c2 = clock.clone();
    let provider = LoopbackProvider::start(move |r: &Request<'_>| {
        // Each request takes twenty simulated seconds.
        c2.advance(20 * SECOND);
        let model = r.body["model"].as_str().unwrap_or("").to_string();
        let turn = request_turn(r.body).unwrap_or(0);
        let step = step_of(r.body);
        if turn == PROVIDER_429_TURN && model == LADDER[0] && step == 0 {
            return Reply::provider_429("Upstream", 1);
        }
        if turn == PLATFORM_429_TURN && step == 0 {
            return Reply::platform_429(20, c2.now() + 3 * MINUTE, 1);
        }
        let bytes = r.body.to_string().len() as u64;
        let served = Served {
            model: format!("{model}-served"),
            prompt: bytes / 4,
            cached: (r.n as u64 % 3) * 7,
            cache_write: (r.n as u64 % 2) * 5,
            completion: 11,
            cost_usd: 0.0,
        };
        let has_tools = r.body.get("tools").is_some();
        let (text, calls) = if has_tools {
            script(turn, step)
        } else {
            (Some("the last step: tools withdrawn".into()), Vec::new())
        };
        let calls: Vec<(String, &str, Value)> = calls
            .into_iter()
            .enumerate()
            .map(|(i, (n, a))| (format!("call_{}_{}_{i}", r.n, step), n, a))
            .collect();
        let refs: Vec<(&str, &str, Value)> = calls
            .iter()
            .map(|(id, n, a)| (id.as_str(), *n, a.clone()))
            .collect();
        Reply::completion("Upstream", text.as_deref(), &refs, served)
    });

    let mut sc = scenario("gate-n", 31);
    let workspace = sc.dir.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let long: String = (0..400)
        .map(|i| format!("line {i:04} of a long file\n"))
        .collect();
    std::fs::write(workspace.join("long.txt"), &long).unwrap();
    sc.config.engine = "agent".into();
    sc.config.ladder = LADDER.iter().map(|s| s.to_string()).collect();
    sc.config.epoch_budget_tokens = 9_000;
    sc.memory = false;
    sc.max_turns = Some(24);
    sc.clock = Some(clock.clone());
    let engine = AgentEngine::new(
        AdapterConfig::new(&provider.url, "test-key-not-a-secret", &workspace),
        clock.clone(),
    )
    .expect("engine");
    sc.engine = Some(Arc::new(engine));
    let out = sim::run(sc);
    let seen = provider.seen();
    if std::env::var("GATE_DEBUG").is_ok() {
        for l in &out.lines {
            if matches!(
                l.kind.as_str(),
                "turn.ended"
                    | "model.switch"
                    | "degraded"
                    | "degraded.ended"
                    | "turn.started"
                    | "epoch.rollover"
            ) {
                eprintln!(
                    "{} {} {}",
                    l.seq,
                    l.kind,
                    serde_json::to_string(&l.to_value())
                        .unwrap()
                        .chars()
                        .take(400)
                        .collect::<String>()
                );
            }
        }
    }
    let g = gates::g_n(&out.lines, &seen);
    assert_gate(&g);
}
