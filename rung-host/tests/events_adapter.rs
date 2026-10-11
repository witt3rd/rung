//! H5: the engine adapter streams partial text and tool progress.
//!
//! The real adapter runs the host against a scripted provider on loopback
//! that answers in streamed chunks: the answer text in several pieces (with a
//! secret split across two of them), and a tool call whose arguments arrive in
//! two pieces. A subscriber on the host's bus sees the frames; the record is
//! what it always was.

mod common;

use std::sync::Arc;

use common::*;
use rung_host::adapter::{AdapterConfig, AgentEngine};
use rung_host::clock::{Clock, SECOND, SimClock};
use rung_host::gates::{Served, request_turn};
use rung_host::redact::Redactor;
use rung_host::sim::http::{LoopbackProvider, Reply, Request};
use rung_host::sim::{self, SIM_START};
use rung_host::stream::{LiveBus, Partial, PartialRedactor, What};
use serde_json::{Value, json};

const LADDER: [&str; 1] = ["test/alpha:free"];
const SECRET: &str = "plainsentinelh5-0123456789";

fn chunk(delta: Value) -> Value {
    json!({"id": "gen-1", "provider": "Upstream", "model": "test/alpha:free-served",
           "object": "chat.completion.chunk", "created": 1_790_985_600,
           "choices": [{"index": 0, "delta": delta, "finish_reason": null}]})
}

/// `base` with its content or tool chunk replaced by `mine` (several chunks).
fn reshape(base: Reply, mine: Vec<Value>) -> Reply {
    let parts: Vec<&str> = base.body.split("\n\n").collect();
    let mut body = String::from(parts[0]);
    body.push_str("\n\n");
    for c in mine {
        body.push_str(&format!("data: {c}\n\n"));
    }
    for p in &parts[2..] {
        if !p.is_empty() {
            body.push_str(p);
            body.push_str("\n\n");
        }
    }
    Reply { body, ..base }
}

fn served(model: &str) -> Served {
    Served {
        model: format!("{model}-served"),
        prompt: 100,
        cached: 0,
        cache_write: 0,
        completion: 11,
        cost_usd: 0.0,
    }
}

/// Tool results so far in the turn.
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

struct Run {
    lines: Vec<rung_host::record::Line>,
    partials: Vec<Partial>,
}

/// One host turn against the scripted provider, with a subscriber on the bus
/// (or none).
fn run(tag: &str, subscribe: bool) -> Run {
    sim::test_timeout(300);
    let clock = Arc::new(SimClock::new(SIM_START));
    let c2 = clock.clone();
    let provider = LoopbackProvider::start(move |r: &Request<'_>| {
        c2.advance(20 * SECOND);
        let model = r.body["model"].as_str().unwrap_or("").to_string();
        let turn = request_turn(r.body).unwrap_or(0);
        if turn != 1 {
            return Reply::completion("Upstream", Some("nothing more"), &[], served(&model));
        }
        let (a, b) = SECRET.split_at(9);
        if step_of(r.body) == 0 {
            let base = Reply::completion(
                "Upstream",
                None,
                &[("call_1", "ws_list", json!({"path": ""}))],
                served(&model),
            );
            reshape(
                base,
                vec![
                    chunk(json!({"role": "assistant", "content": "Looking at the "})),
                    chunk(json!({"content": format!("workspace. The key is {a}")})),
                    chunk(json!({"content": format!("{b} so be careful.")})),
                    chunk(
                        json!({"tool_calls": [{"index": 0, "id": "call_1", "type": "function",
                        "function": {"name": "ws_list", "arguments": "{\"pa"}}]}),
                    ),
                    chunk(json!({"tool_calls": [{"index": 0,
                        "function": {"arguments": "th\": \"\"}"}}]})),
                ],
            )
        } else {
            let base = Reply::completion("Upstream", Some("x"), &[], served(&model));
            reshape(
                base,
                vec![
                    chunk(json!({"role": "assistant", "content": "All listed. "})),
                    chunk(json!({"content": "Nothing else to do."})),
                ],
            )
        }
    });
    let mut sc = scenario(tag, 41);
    let workspace = sc.dir.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(workspace.join("a.txt"), "hello").unwrap();
    sc.config.engine = "agent".into();
    sc.config.ladder = LADDER.iter().map(|s| s.to_string()).collect();
    sc.memory = false;
    sc.max_turns = Some(1);
    sc.clock = Some(clock.clone());
    let bus = LiveBus::new();
    let mut rx = subscribe.then(|| bus.subscribe());
    sc.live = Some(bus);
    let ac = AdapterConfig::new(&provider.url, "test-key-not-a-secret", &workspace);
    sc.engine = Some(Arc::new(
        AgentEngine::new(ac, clock.clone() as Arc<dyn Clock>).expect("engine"),
    ));
    let out = sim::run(sc);
    let mut partials = Vec::new();
    if let Some(rx) = rx.as_mut() {
        while let Ok(p) = rx.try_recv() {
            partials.push((*p).clone());
        }
    }
    Run {
        lines: out.lines,
        partials,
    }
}

fn texts(ps: &[Partial], call: u32) -> String {
    ps.iter()
        .filter(|p| p.call == call)
        .filter_map(|p| match &p.what {
            What::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_adapter_streams_partial_text_and_tool_progress_to_a_subscriber() {
    let r = run("h5-adapter", true);
    let ps = &r.partials;
    assert!(!ps.is_empty(), "the adapter published nothing");
    assert!(ps.iter().all(|p| p.turn == 1), "frames carry their turn");
    // the text of each call, in pieces, in order
    assert_eq!(
        texts(ps, 0),
        format!("Looking at the workspace. The key is {SECRET} so be careful.")
    );
    assert_eq!(texts(ps, 1), "All listed. Nothing else to do.");
    assert!(
        ps.iter()
            .filter(|p| p.call == 0 && matches!(p.what, What::Text { .. }))
            .count()
            >= 3,
        "text arrived as one piece, not as it was formed"
    );
    // the tool call: named, its arguments in pieces, then run, then done
    let kinds: Vec<&str> = ps
        .iter()
        .map(|p| match &p.what {
            What::Text { .. } => "text",
            What::ToolCall { .. } => "tool_call",
            What::ToolInput { .. } => "tool_input",
            What::ToolRun { .. } => "tool_run",
            What::ToolDone { .. } => "tool_done",
            What::BlockStart { .. } => "block_start",
            What::BlockEnd { .. } => "block_end",
            What::TurnEnd => "turn_end",
            What::Behind { .. } => "behind",
        })
        .collect();
    let pos = |k: &str| {
        kinds
            .iter()
            .position(|x| *x == k)
            .unwrap_or_else(|| panic!("no {k}"))
    };
    assert!(pos("text") < pos("tool_call"));
    assert!(pos("tool_call") < pos("tool_input"));
    assert!(pos("tool_input") < pos("tool_run"));
    assert!(pos("tool_run") < pos("tool_done"));
    assert_eq!(kinds.last(), Some(&"turn_end"));
    let call = ps
        .iter()
        .find_map(|p| match &p.what {
            What::ToolCall { id, name } => Some((id.clone(), name.clone())),
            _ => None,
        })
        .unwrap();
    assert_eq!(call, ("call_1".to_string(), "ws_list".to_string()));
    let input: String = ps
        .iter()
        .filter_map(|p| match &p.what {
            What::ToolInput { delta, .. } => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        serde_json::from_str::<Value>(&input).unwrap(),
        json!({"path": ""})
    );
    let done = ps
        .iter()
        .find_map(|p| match &p.what {
            What::ToolDone { name, ok, output } => Some((name.clone(), *ok, output.clone())),
            _ => None,
        })
        .unwrap();
    assert_eq!((done.0.as_str(), done.1), ("ws_list", true));
    assert!(done.2.contains("a.txt"), "the tool's output is whole");
    // text blocks and tool arguments each say when they are complete
    assert!(kinds.contains(&"block_end"));
}

#[test]
fn streaming_changes_nothing_in_the_record() {
    let watched = run("h5-adapter-rec-a", true);
    let unwatched = run("h5-adapter-rec-b", false);
    // `ttft_ms` is a wall-clock measure of the real socket: it differs run to run
    let strip = |r: &Run| -> Vec<Value> {
        r.lines
            .iter()
            .map(|l| {
                let mut v = l.without_wall();
                if let Some(m) = v.as_object_mut() {
                    m.remove("ttft_ms");
                }
                v
            })
            .collect()
    };
    assert_eq!(
        strip(&watched),
        strip(&unwatched),
        "a subscriber changed the record"
    );
    // and no partial frame is in it: no line has a frame's kind or fields
    for l in &watched.lines {
        assert!(
            !matches!(
                l.kind.as_str(),
                "delta"
                    | "text"
                    | "tool_input"
                    | "tool_run"
                    | "tool_done"
                    | "block_end"
                    | "turn_end"
            ),
            "a partial frame was recorded as {}",
            l.kind
        );
    }
    // the final text is recorded as before
    let ended = watched
        .lines
        .iter()
        .find(|l| l.kind == "turn.ended")
        .unwrap();
    assert_eq!(ended.str("final_text"), "All listed. Nothing else to do.");
}

#[test]
fn the_secret_split_across_the_models_chunks_never_leaves_through_the_redactor() {
    let r = run("h5-adapter-red", true);
    let mut pr = PartialRedactor::new(Redactor::new().with_secret(SECRET));
    let mut wire = String::new();
    let mut said = String::new();
    for p in r.partials.iter().filter(|p| p.call == 0).cloned() {
        for f in pr.push(p) {
            wire.push_str(&f.to_string());
            wire.push('\n');
            if f["kind"] == "text" {
                said.push_str(f["text"].as_str().unwrap());
            }
        }
    }
    assert!(!wire.contains(SECRET), "a secret left in a partial frame");
    assert!(
        said == "Looking at the workspace. The key is [redacted] so be careful.",
        "the text around the removed key is not what was expected"
    );
}
