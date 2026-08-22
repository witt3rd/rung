//! Streaming trace emission for `--stream`.
//!
//! Emits NDJSON to stdout as the agent loop runs, one event per line:
//!
//! ```text
//! {"type":"text","content":"..."}
//! {"type":"thinking","content":"..."}
//! {"type":"tool_use","name":"...","input":{...},"id":"..."}
//! {"type":"tool_result","tool_use_id":"...","content":"...","is_error":bool}
//! {"type":"result","response":{task_id,text,status,api_calls,usage,model}}
//! ```
//!
//! The final `result` line is always emitted last, carrying the same fields
//! as the `--json` `Outcome` plus token `usage` and the routed `model`.

use std::collections::{HashMap, VecDeque};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use rung_std::agent::AgentResult;
use rung_std::llm::{
    ContentBlockDelta, ContentBlockStart, StreamEvent, StreamListener, ToolDefinition, Usage,
};
use rung_std::tools::Toolset;

use serde_json::{Value, json};

/// Shared emitter. Both the [`StreamListener`] (LLM tokens + tool_use) and
/// the observing [`Toolset`] (tool_results) write into this one object.
pub struct Emitter {
    out: Mutex<io::Stdout>,
    // index -> (id, name) of an in-progress tool_use block
    tools: Mutex<HashMap<usize, (String, String)>>,
    // index -> accumulated input JSON of an in-progress tool_use block
    inputs: Mutex<HashMap<usize, String>>,
    // FIFO of completed tool_use ids, in declaration order, for result pairing
    pending: Mutex<VecDeque<(String, String)>>,
}

impl Emitter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            out: Mutex::new(io::stdout()),
            tools: Mutex::new(HashMap::new()),
            inputs: Mutex::new(HashMap::new()),
            pending: Mutex::new(VecDeque::new()),
        })
    }

    fn write(&self, line: Value) {
        let mut out = self.out.lock().unwrap();
        let _ = writeln!(out, "{}", line);
        let _ = out.flush();
    }

    /// Pop the next completed tool_use (id, name) in declaration order.
    fn next_tool(&self) -> Option<(String, String)> {
        self.pending.lock().unwrap().pop_front()
    }

    fn emit_tool_result(&self, id: &str, content: &str, is_error: bool) {
        self.write(json!({
            "type": "tool_result",
            "tool_use_id": id,
            "content": content,
            "is_error": is_error,
        }));
    }

    /// Final success line. Call once when the loop returns `Ok`.
    pub fn emit_result(
        &self,
        task_id: &str,
        r: &AgentResult,
        model: &str,
        isolation: Option<&str>,
    ) {
        self.write(json!({
            "type": "result",
            "response": {
                "task_id": task_id,
                "text": r.final_response,
                "status": "completed",
                "api_calls": r.api_calls_made,
                "usage": usage_json(&r.usage),
                "model": model,
                "isolation_path": isolation,
            }
        }));
    }

    /// Final error line. Call once when the loop returns `Err`.
    pub fn emit_error(&self, task_id: &str, reason: &str, model: &str) {
        self.write(json!({
            "type": "result",
            "response": {
                "task_id": task_id,
                "text": "",
                "status": "error",
                "error": reason,
                "api_calls": 0,
                "model": model,
                "isolation_path": Value::Null,
            }
        }));
    }
}

fn usage_json(u: &Usage) -> Value {
    json!({
        "input_tokens": u.input_tokens,
        "output_tokens": u.output_tokens,
        "non_cached_input_tokens": u.non_cached_input_tokens,
        "cache_read_input_tokens": u.cache_read_input_tokens,
        "cache_creation_input_tokens": u.cache_creation_input_tokens,
        "thinking_tokens": u.thinking_tokens,
        "service_tier": u.service_tier,
    })
}

impl StreamListener for Emitter {
    fn on_event(&self, event: StreamEvent) {
        match event {
            StreamEvent::ContentBlockStart {
                index,
                block: ContentBlockStart::ToolUse { id, name },
            } => {
                self.tools.lock().unwrap().insert(index, (id, name));
            }
            StreamEvent::ContentBlockStart { .. } => {}
            StreamEvent::ContentBlockDelta { index, delta } => match delta {
                ContentBlockDelta::TextDelta(t) if !t.is_empty() => {
                    self.write(json!({"type": "text", "content": t}));
                }
                ContentBlockDelta::ThinkingDelta(t) if !t.is_empty() => {
                    self.write(json!({"type": "thinking", "content": t}));
                }
                ContentBlockDelta::InputJsonDelta(j) => {
                    let mut m = self.inputs.lock().unwrap();
                    m.entry(index).or_default().push_str(&j);
                }
                _ => {}
            },
            StreamEvent::ContentBlockStop { index } => {
                let tool = self.tools.lock().unwrap().remove(&index);
                if let Some((id, name)) = tool {
                    let input_json = self
                        .inputs
                        .lock()
                        .unwrap()
                        .remove(&index)
                        .unwrap_or_default();
                    let input: Value = if input_json.is_empty() {
                        json!({})
                    } else {
                        serde_json::from_str(&input_json).unwrap_or(json!({}))
                    };
                    self.pending
                        .lock()
                        .unwrap()
                        .push_back((id.clone(), name.clone()));
                    self.write(json!({
                        "type": "tool_use",
                        "name": name,
                        "input": input,
                        "id": id,
                    }));
                }
            }
            _ => {}
        }
    }
}

/// Wraps a [`Toolset`] to emit `tool_result` lines as tools execute.
pub struct ObservingToolset {
    pub inner: Arc<dyn Toolset>,
    pub emitter: Arc<Emitter>,
}

impl std::fmt::Debug for ObservingToolset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObservingToolset").finish()
    }
}

impl Toolset for ObservingToolset {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.inner.definitions()
    }

    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        match self.inner.execute(name, input) {
            Ok(s) => {
                if let Some((id, _)) = self.emitter.next_tool() {
                    self.emitter.emit_tool_result(&id, &s, false);
                }
                Ok(s)
            }
            Err(e) => {
                if let Some((id, _)) = self.emitter.next_tool() {
                    self.emitter.emit_tool_result(&id, &e, true);
                }
                Err(e)
            }
        }
    }
}
