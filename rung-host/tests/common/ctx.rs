#![allow(dead_code)]
//! Pieces shared by the request-record tests (S4): a scripted engine that
//! makes a chosen number of model calls a turn and remembers what it was
//! handed, a memory provider that recalls one long text, and readers of the
//! record and of the context door.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rung_agent_core::engine::CallUsage;
use rung_host::canon;
use rung_host::clock::{Clock, SimClock};
use rung_host::context::{ContextQuery, open_dir};
use rung_host::engine::{CallRecord, Ended, EngineTurn, TurnEngine, TurnRequest};
use rung_host::inbox::{Item, MemorySource};
use rung_host::record::{Line, Record};
use rung_host::redact::Redactor;
use rung_host::sim::{self, SIM_START};
use rung_memory::{
    Budget, Capability, Charged, Cue, Kept, MemoryProvider, Miss, Observation, Reach, Recalled,
    Record as MemRecord, RecordId, Scope,
};
use rung_std::llm::{ChatMessage, MessageContentBlock, ToolDefinition, Usage};
use serde_json::{Value, json};

/// What a request was made of when the engine was handed it.
#[derive(Clone)]
pub struct Seen {
    pub turn: u64,
    pub tools: Vec<ToolDefinition>,
    pub system: String,
    pub messages: Vec<ChatMessage>,
}

/// An engine that makes `calls` model calls a turn, each a tool call and its
/// result, then answers; and remembers the request it was handed.
pub struct Script {
    pub calls: usize,
    pub clock: Arc<dyn Clock>,
    pub seen: Mutex<Vec<Seen>>,
}

impl Script {
    pub fn new(calls: usize, clock: Arc<dyn Clock>) -> Arc<Self> {
        Arc::new(Self {
            calls,
            clock,
            seen: Mutex::new(Vec::new()),
        })
    }
}

pub fn scripted_messages(turn: u64, calls: usize) -> Vec<ChatMessage> {
    let mut out = Vec::new();
    for i in 0..calls.saturating_sub(1) {
        out.push(ChatMessage::assistant_with_blocks(vec![
            MessageContentBlock::ToolUse {
                id: format!("c{turn}-{i}"),
                name: "look".into(),
                input: json!({"i": i}),
                cache: None,
            },
        ]));
        out.push(ChatMessage::user_with_blocks(vec![
            MessageContentBlock::ToolResult {
                tool_use_id: format!("c{turn}-{i}"),
                content: format!("result {turn}-{i}"),
                images: Vec::new(),
                is_error: false,
                cache: None,
            },
        ]));
    }
    out.push(ChatMessage::assistant(format!("done {turn}")));
    out
}

impl TurnEngine for Script {
    fn name(&self) -> &str {
        "script"
    }
    fn turn(&self, req: TurnRequest) -> EngineTurn {
        self.seen.lock().unwrap().push(Seen {
            turn: req.turn,
            tools: req.tools.definitions(),
            system: req.thread.system_prompt.clone(),
            messages: req.thread.messages.clone(),
        });
        let mut calls = Vec::new();
        for i in 0..self.calls {
            self.clock.advance(5);
            let prompt = 1000 + 50 * i as u32;
            // the first call finds the stable layer cached, the next find
            // the previous request cached
            let cached = if i == 0 { 600 } else { prompt - 50 };
            let mut u = Usage::from_openai(prompt, 20, cached, 0);
            u.cost_usd = Some(0.0);
            calls.push(CallRecord {
                usage: CallUsage {
                    model: req.model.clone(),
                    usage: Some(u),
                },
                provider: "script".into(),
                latency_ms: 5,
            });
        }
        EngineTurn {
            messages: scripted_messages(req.turn, self.calls),
            final_text: Some(format!("done {}", req.turn)),
            calls,
            failure: None,
            ended: Ended::Completed,
            rewritten: false,
        }
    }
}

pub fn clock() -> Arc<dyn Clock> {
    Arc::new(SimClock::new(SIM_START))
}

pub fn record_lines(dir: &Path) -> Vec<Line> {
    Record::read_dir(dir.join("record")).unwrap()
}

/// Every byte of the record, as it is on disk.
pub fn record_bytes(dir: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(dir.join("record"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    for f in files {
        out.extend(std::fs::read(f).unwrap());
    }
    out
}

pub fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

/// The context door for one call of one turn, as the JSON it answers.
pub fn door(
    dir: &Path,
    turn: u64,
    call: Option<u64>,
    red: &Redactor,
) -> Result<Value, (u16, String)> {
    let page = open_dir(dir, turn, &ContextQuery { call }, red, Vec::new())
        .map_err(|e| (e.status, e.code.to_string()))?;
    let mut out = Vec::new();
    page.write_json(&mut out).unwrap();
    Ok(serde_json::from_slice(&out).expect("the door answers one JSON document"))
}

pub fn band<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["bands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["band"] == name)
        .unwrap_or_else(|| panic!("no {name} band in {:?}", names(v)))
}

pub fn names(v: &Value) -> Vec<String> {
    v["bands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["band"].as_str().unwrap().to_string())
        .collect()
}

/// A scenario on `dir` driven by a scripted engine.
pub fn scripted(dir: &Path, calls: usize, turns: u64) -> (sim::Scenario, Arc<Script>) {
    let mut sc = sim::Scenario::new(dir, 5);
    let clk = clock();
    sc.clock = Some(clk.clone());
    sc.memory = false;
    sc.max_turns = Some(turns);
    let script = Script::new(calls, clk);
    sc.engine = Some(script.clone());
    (sc, script)
}

/// The messages the model saw at call `k` of `turn` (1-based), rebuilt from
/// what the engine was handed at the turn's start and what it added.
pub fn wire(seen: &Seen, calls: usize, k: usize) -> Vec<ChatMessage> {
    let mut m = seen.messages.clone();
    let added = scripted_messages(seen.turn, calls);
    m.extend(added.into_iter().take(2 * (k - 1)));
    m
}

/// A memory provider that recalls one record of a given text.
#[derive(Debug)]
pub struct Big {
    pub text: String,
}

impl MemoryProvider for Big {
    fn name(&self) -> &str {
        "big"
    }
    fn capability(&self) -> Capability {
        Capability {
            recall: true,
            retain: true,
            tools: false,
        }
    }
    fn budget(&self) -> Budget {
        Budget::default()
    }
    fn recall(&self, scope: &Scope, _cue: &Cue) -> Result<Charged<Vec<Recalled>>, Miss> {
        Ok(Charged::new(vec![Recalled {
            record: MemRecord {
                id: RecordId::new("big-1"),
                scope: scope.clone(),
                text: self.text.clone(),
                observed_at: None,
                attrs: BTreeMap::new(),
            },
            reach: Reach::Hit { score: 1.0 },
        }]))
    }
    fn retain(&self, _: &Scope, _: &Observation) -> Result<Charged<Kept>, Miss> {
        Ok(Charged::new(Kept::Stored(RecordId::new("kept-1"))))
    }
}

/// `n` lines of 99 characters, each numbered: a long recalled text.
pub fn long_recall(n: usize, plant: Option<(usize, &str)>) -> String {
    (0..n)
        .map(|i| match plant {
            Some((at, s)) if at == i => {
                format!("rl{i:05} {s} {}", "x".repeat(99 - 9 - s.len() - 1))
            }
            _ => format!("rl{i:05} {}", "x".repeat(99 - 8)),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn owner_says(at: i64, text: &str) -> Box<MemorySource> {
    let mut m = MemorySource::default();
    m.push(Item::message(
        "q-1",
        rung_host::inbox::Role::Owner,
        "owner",
        at,
        text,
    ));
    Box::new(m)
}

/// Append one line to a stopped record, as a host that ran before would have.
pub fn append_line(dir: &Path, kind: &str, body: Value) {
    let last = Record::last_line(dir.join("record")).unwrap().unwrap();
    let mut m = body.as_object().cloned().unwrap();
    m.insert("seq".into(), (last.seq + 1).into());
    m.insert("at".into(), (last.at + 1).into());
    m.insert("kind".into(), kind.into());
    let mut files: Vec<_> = std::fs::read_dir(dir.join("record"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(files.last().unwrap())
        .unwrap();
    writeln!(f, "{}", canon::string(&Value::Object(m))).unwrap();
}
