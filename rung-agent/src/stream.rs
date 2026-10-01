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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rung_std::llm::{
    ContentBlockDelta, ContentBlockStart, StreamEvent, StreamListener, ToolDefinition, Usage,
};
use rung_std::tools::{ToolOutput, Toolset};

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
    pub fn emit_result(&self, out: &crate::run::Outcome, usage: &Usage, model: &str) {
        let mut response = json!({
            "task_id": out.task_id,
            "text": out.text,
            "status": out.status,
            "api_calls": out.api_calls,
            "usage": usage_json(usage),
            "model": model,
            "isolation_path": out.isolation_path,
        });
        if let Some(tc) = &out.turn_check {
            response["turn_check"] = json!(tc);
        }
        self.write(json!({
            "type": "result",
            "response": response,
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
        let result = self.inner.execute(name, input);
        self.observe(result.as_deref().map_err(String::as_str));
        result
    }

    fn execute_output(&self, name: &str, input: &Value) -> Result<ToolOutput, String> {
        let result = self.inner.execute_output(name, input);
        self.observe(
            result
                .as_ref()
                .map(|o| o.text.as_str())
                .map_err(String::as_str),
        );
        result
    }
}

impl ObservingToolset {
    /// One `tool_result` line. Text only: image data is not streamed.
    fn observe(&self, result: Result<&str, &str>) {
        if let Some((id, _)) = self.emitter.next_tool() {
            match result {
                Ok(s) => self.emitter.emit_tool_result(&id, s, false),
                Err(e) => self.emitter.emit_tool_result(&id, e, true),
            }
        }
    }
}

/// Start/finish hooks around [`Toolset::execute`]. ACP uses this to emit
/// `session/update` tool calls; IDs are generated here (not the LLM's).
pub trait ToolNotify: Send + Sync {
    fn started(&self, id: &str, name: &str, input: &Value);
    fn finished(&self, id: &str, name: &str, result: Result<&str, &str>);
}

pub struct NotifyingToolset<N: ToolNotify> {
    pub inner: Arc<dyn Toolset>,
    pub notify: N,
    seq: AtomicU64,
}

impl<N: ToolNotify> NotifyingToolset<N> {
    pub fn new(inner: Arc<dyn Toolset>, notify: N) -> Self {
        Self {
            inner,
            notify,
            seq: AtomicU64::new(0),
        }
    }
}

impl<N: ToolNotify> std::fmt::Debug for NotifyingToolset<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotifyingToolset").finish()
    }
}

impl<N: ToolNotify> Toolset for NotifyingToolset<N> {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.inner.definitions()
    }

    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        let id = format!("tool-{}", self.seq.fetch_add(1, Ordering::Relaxed));
        self.notify.started(&id, name, input);
        let result = self.inner.execute(name, input);
        self.notify
            .finished(&id, name, result.as_deref().map_err(String::as_str));
        result
    }

    fn execute_output(&self, name: &str, input: &Value) -> Result<ToolOutput, String> {
        let id = format!("tool-{}", self.seq.fetch_add(1, Ordering::Relaxed));
        self.notify.started(&id, name, input);
        let result = self.inner.execute_output(name, input);
        let text = result
            .as_ref()
            .map(|o| o.text.as_str())
            .map_err(String::as_str);
        self.notify.finished(&id, name, text);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rung_std::tools::{Tool, ToolCollection, ToolRoster};

    #[derive(Debug)]
    struct Echo;

    impl Tool for Echo {
        fn name(&self) -> &'static str {
            "echo"
        }
        fn description(&self) -> &'static str {
            "echo"
        }
        fn input_schema(&self) -> Value {
            json!({"type": "object"})
        }
        fn execute(&self, input: &Value) -> Result<String, String> {
            Ok(input.to_string())
        }
    }

    struct Rec(Arc<Mutex<Vec<String>>>);

    impl ToolNotify for Rec {
        fn started(&self, id: &str, name: &str, _input: &Value) {
            self.0.lock().unwrap().push(format!("start {id} {name}"));
        }
        fn finished(&self, id: &str, name: &str, result: Result<&str, &str>) {
            self.0
                .lock()
                .unwrap()
                .push(format!("end {id} {name} {}", result.is_ok()));
        }
    }

    #[derive(Debug)]
    struct Frame;

    impl Tool for Frame {
        fn name(&self) -> &'static str {
            "frame"
        }
        fn description(&self) -> &'static str {
            "frame"
        }
        fn input_schema(&self) -> Value {
            json!({"type": "object"})
        }
        fn execute(&self, _input: &Value) -> Result<String, String> {
            Ok("text only".into())
        }
        fn execute_output(&self, _input: &Value) -> Result<ToolOutput, String> {
            Ok(
                ToolOutput::text("frame").with_image(rung_std::llm::ImageSource::base64(
                    "image/png",
                    "iVBORw0KGgo=",
                )),
            )
        }
    }

    #[test]
    fn wrappers_forward_tool_images() {
        let mut c = ToolCollection::new("t");
        c.admit(Frame);
        let mut r = ToolRoster::new();
        r.add(c);
        let log = Arc::new(Mutex::new(Vec::new()));
        let notifying = NotifyingToolset::new(Arc::new(r), Rec(log.clone()));
        let observing = ObservingToolset {
            inner: Arc::new(notifying),
            emitter: Emitter::new(),
        };
        let out = observing.execute_output("frame", &json!({})).unwrap();
        assert_eq!(out.text, "frame");
        assert_eq!(out.images.len(), 1);
        assert_eq!(
            *log.lock().unwrap(),
            vec!["start tool-0 frame", "end tool-0 frame true"]
        );
    }

    #[test]
    fn notifying_toolset_emits_start_then_finish() {
        let mut c = ToolCollection::new("t");
        c.admit(Echo);
        let mut r = ToolRoster::new();
        r.add(c);
        let log = Arc::new(Mutex::new(Vec::new()));
        let set = NotifyingToolset::new(Arc::new(r), Rec(log.clone()));
        set.execute("echo", &json!({"a": 1})).unwrap();
        let got = log.lock().unwrap().clone();
        assert_eq!(got, vec!["start tool-0 echo", "end tool-0 echo true"]);
    }
}
